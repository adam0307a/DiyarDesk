import groovy.json.JsonSlurper

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val versionFile = rootProject.file("../tools/version.json")
@Suppress("UNCHECKED_CAST")
val appVersionName: String = if (versionFile.exists()) {
    val parsed = JsonSlurper().parse(versionFile) as Map<String, Any>
    parsed["version"]?.toString() ?: "0.2.0"
} else {
    "0.2.0"
}
val versionParts = appVersionName.split('.').mapNotNull { it.toIntOrNull() }
val computedVersionCode = (versionParts.getOrElse(0) { 0 } * 10000) +
    (versionParts.getOrElse(1) { 2 } * 100) +
    versionParts.getOrElse(2) { 0 }

val generatedAssetsDir = layout.buildDirectory.dir("generated/prototypeAssets")

val syncPrototypeAssets by tasks.registering(Sync::class) {
    from(rootProject.file("../prototype")) {
        exclude("serve.mjs", ".DS_Store")
        into("prototype")
    }
    into(generatedAssetsDir)
}

tasks.matching {
    it.name != "syncPrototypeAssets" &&
        (it.name.contains("Assets", ignoreCase = true) || it.name.contains("lint", ignoreCase = true))
}.configureEach {
    dependsOn(syncPrototypeAssets)
}

android {
    namespace = "com.diyardesk.tablet"
    compileSdk = 35

    sourceSets.named("main") {
        assets.srcDir(generatedAssetsDir)
    }

    defaultConfig {
        applicationId = "com.diyardesk.tablet"
        minSdk = 29
        targetSdk = 35
        versionCode = computedVersionCode
        versionName = appVersionName

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            signingConfig = signingConfigs.getByName("debug")
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
        debug {
            applicationIdSuffix = ".debug"
        }
    }

    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "armeabi-v7a", "x86_64")
            isUniversalApk = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    testOptions {
        unitTests.isIncludeAndroidResources = false
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.webkit:webkit:1.12.1")
    implementation("androidx.activity:activity-ktx:1.9.3")

    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}
