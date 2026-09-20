plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

/** `version = "x.y.z"` from the workspace's Cargo.toml. */
val workspaceVersion: String = run {
    val toml = rootProject.file("../Cargo.toml").readText()
    val inPackage = toml.substringAfter("[workspace.package]")
    Regex("(?m)^version\\s*=\\s*\"([^\"]+)\"").find(inPackage)?.groupValues?.get(1)
        ?: error("no [workspace.package] version in ../Cargo.toml")
}

/**
 * The version code a version name orders as: major, minor and patch in
 * three fields of a thousand, so 0.1.0 is 1000 and 1.2.3 is 1_002_003.
 * A field of a thousand or more would collide with the next; the build
 * refuses it rather than upload something Play orders wrongly.
 */
fun versionCodeOf(name: String): Int {
    val parts = name.substringBefore('-').split('.').map { it.toInt() }
    require(parts.size == 3) { "version $name is not major.minor.patch" }
    require(parts.all { it in 0..999 }) { "version $name has a field of 1000 or more" }
    return parts[0] * 1_000_000 + parts[1] * 1_000 + parts[2]
}

/** The upload key's four settings, all present or none. */
val uploadKey: Map<String, String>? = run {
    val names = listOf(
        "SIGIL_UPLOAD_KEYSTORE",
        "SIGIL_UPLOAD_KEYSTORE_PASSWORD",
        "SIGIL_UPLOAD_KEY_ALIAS",
        "SIGIL_UPLOAD_KEY_PASSWORD",
    )
    val set = names.mapNotNull { n -> System.getenv(n)?.takeIf { it.isNotEmpty() }?.let { n to it } }.toMap()
    when (set.size) {
        0 -> null
        names.size -> set
        else -> error("upload key: set all of ${names.joinToString()} or none (have ${set.keys.joinToString()})")
    }
}

android {
    namespace = "org.squic.sigil"
    // 36: Google Play takes nothing older than Android 16 for a new app or
    // an update since 31 August 2026. Move compileSdk and targetSdk
    // together, and ci.yml's sdkmanager line with them.
    compileSdk = 36
    // The NDK Gradle strips native libraries with. Without it Gradle warns
    // "unable to strip" and packages them as they are; scripts/build-apk
    // strips first anyway, so this is the belt to that's braces.
    ndkVersion = (System.getenv("ANDROID_NDK_VERSION") ?: "27.2.12479018")

    defaultConfig {
        applicationId = "org.squic.sigil"
        // 26: notification channels and the foreground-service shapes a call
        // needs. Nothing here wants newer, and a phone older than 2017 is
        // not a phone somebody is putting a new key on.
        minSdk = 26
        targetSdk = 36
        // One version, the workspace's: Cargo.toml's [workspace.package]
        // version names the release, and the code Play orders uploads by is
        // derived from it, so a bump is one edit and a tag. Play rejects an
        // upload whose code does not exceed the last, which is what makes a
        // forgotten bump fail loudly.
        versionName = workspaceVersion
        versionCode = versionCodeOf(workspaceVersion)
        ndk {
            // What `cargo ndk` built. One ABI per build; scripts/build-apk
            // passes the others through the same property.
            abiFilters += (project.findProperty("sigil.abi") as String).split(",")
        }
    }

    // The Rust library lands here, built by scripts/build-apk before Gradle
    // runs -- Gradle does not drive cargo, so a build with a stale library
    // is a build somebody forgot the first step of. The script checks.
    sourceSets["main"].jniLibs.srcDirs("src/main/jniLibs")

    // The FCM gateway, if any, as a build constant: `-Psigil.wakeProxy=…`
    // or the property in gradle.properties. Empty means no embedded
    // distributor is offered.
    defaultConfig.buildConfigField(
        "String",
        "WAKE_PROXY",
        "\"${(project.findProperty("sigil.wakeProxy") as String?) ?: ""}\""
    )
    buildFeatures {
        buildConfig = true
    }

    // One debug key, committed: a debug key has no security value, and a
    // fresh one per build makes every build refuse to install over the
    // last ("signatures do not match"). PKCS12 because it was made with
    // OpenSSL on a machine with no Java; the passwords are the standard
    // debug ones.
    //
    // The upload key is not here. It comes from the environment -- the
    // release workflow's secrets, or a shell that exported them -- and
    // only when all four are set. Without them a release build is signed
    // with the debug key, which installs on a phone over adb and is
    // refused by Play, so a build that forgot the key cannot ship by
    // mistake; scripts/build-apk --bundle refuses outright.
    signingConfigs {
        getByName("debug") {
            storeFile = rootProject.file("debug.keystore")
            storeType = "PKCS12"
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }
        uploadKey?.let { key ->
            create("upload") {
                storeFile = file(key.getValue("SIGIL_UPLOAD_KEYSTORE"))
                storeType = "PKCS12"
                storePassword = key.getValue("SIGIL_UPLOAD_KEYSTORE_PASSWORD")
                keyAlias = key.getValue("SIGIL_UPLOAD_KEY_ALIAS")
                keyPassword = key.getValue("SIGIL_UPLOAD_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName(if (uploadKey != null) "upload" else "debug")
        }
    }

    // The bundle Play takes. Native libraries are kept per ABI inside it
    // and Play serves each phone its own, so a second ABI costs a download
    // nothing. Language and density splits are the defaults.
    bundle {
        abi { enableSplit = true }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    // UnifiedPush: the phone chooses its distributor (SIP-45). The connector
    // speaks to whichever one is installed; the embedded FCM distributor is
    // the bridge SIP-47 describes for a phone with nothing else, and is
    // used only when no other distributor is present. Coordinates as the
    // UnifiedPush example application's version catalog names them.
    // 3.3.2, not newer: 3.3.4's AAR metadata demands compile SDK 37, which
    // no released platform provides (sdkmanager has no platforms;android-37).
    implementation("org.unifiedpush.android:connector:3.3.2")
    implementation("org.unifiedpush.android:embedded-fcm-distributor:3.1.0")
}
