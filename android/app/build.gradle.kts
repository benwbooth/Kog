plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "org.kog.player"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.kog.player"
        minSdk = 28
        targetSdk = 35
        versionCode = 1
        versionName = "0.9.41-dev"
    }

    buildTypes {
        release { isMinifyEnabled = false }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures { compose = true }
    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
        // Helper executables are packed as native libraries and need a real
        // executable path in applicationInfo.nativeLibraryDir.
        jniLibs.useLegacyPackaging = true
    }
}

val kogNotices = tasks.register<Copy>("copyKogNotices") {
    val sourceRoot = rootProject.projectDir.parentFile
    from(sourceRoot.resolve("LICENSE"))
    from(sourceRoot.resolve("THIRD_PARTY_NOTICES.md"))
    from(sourceRoot.resolve("LICENSES")) { into("LICENSES") }
    into(layout.buildDirectory.dir("generated/kogNotices"))
}
android.sourceSets["main"].assets.srcDir(layout.buildDirectory.dir("generated/kogNotices"))
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("Assets") }
    .configureEach { dependsOn(kogNotices) }

kotlin {
    compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) }
}

dependencies {
    // Compose 1.12 requires AGP 9 / API 37; 1.11 works with our API 36 toolchain.
    implementation(platform("androidx.compose:compose-bom:2026.04.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.activity:activity-compose:1.11.0")
    implementation("androidx.core:core-ktx:1.17.0")
    implementation("androidx.documentfile:documentfile:1.1.0")
    implementation("androidx.media3:media3-exoplayer:1.9.2")
    implementation("androidx.media3:media3-session:1.9.2")
    implementation("androidx.media3:media3-datasource:1.9.2")
    implementation("io.coil-kt:coil-compose:2.7.0")
}
