pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // UnifiedPush's connector and its embedded FCM distributor.
        maven { url = uri("https://jitpack.io") }
    }
}
rootProject.name = "sigil"
include(":app")
