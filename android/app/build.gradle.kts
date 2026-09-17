plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "org.squic.sigil"
    compileSdk = 35

    defaultConfig {
        applicationId = "org.squic.sigil"
        // 26: notification channels and the foreground-service shapes a call
        // needs. Nothing here wants newer, and a phone older than 2017 is
        // not a phone somebody is putting a new key on.
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
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

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    // UnifiedPush: the phone chooses its distributor (SIP-45). The connector
    // speaks to whichever one is installed; the embedded FCM distributor is
    // the bridge SIP-47 describes for a phone with nothing else, and is
    // used only when no other distributor is present.
    implementation("org.unifiedpush.android:connector:2.5.0")
    implementation("org.unifiedpush.android.embedded_fcm_distributor:embedded_fcm_distributor:2.3.0")
}
