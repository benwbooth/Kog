plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val kogVersion = Regex("""(?m)^version = "([^"]+)"""")
    .find(rootProject.file("../Cargo.toml").readText())!!.groupValues[1]
val versionParts = kogVersion.substringBefore('-').split('.').map(String::toInt)
val kogVersionCode = versionParts[0] * 1_000_000 + versionParts[1] * 1_000 + versionParts[2]
val kogAbis = providers.gradleProperty("kogAbis").getOrElse("arm64-v8a").split(',')

android {
    namespace = "org.kog.player"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.kog.player"
        minSdk = 28
        targetSdk = 35
        testInstrumentationRunner = "org.kog.player.UiContractInstrumentation"
        versionCode = kogVersionCode
        versionName = kogVersion
        ndk { abiFilters += kogAbis }
    }

    sourceSets.getByName("androidTest").assets.srcDir(rootProject.file("../tests/ui-contract"))

    buildTypes {
        debug { versionNameSuffix = "-dev" }
        release {
            isDebuggable = false
            isJniDebuggable = false
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures { compose = true }
    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
        // NativeAudio configures nativeLibraryDir on service startup, so it
        // must exist even though the decoders now run in process.
        jniLibs.useLegacyPackaging = true
    }
}

abstract class CopyKogNotices : Sync() {
    @get:OutputDirectory
    abstract val outputDirectory: DirectoryProperty

    init { into(outputDirectory) }
}

val kogNotices = tasks.register<CopyKogNotices>("copyKogNotices") {
    val sourceRoot = rootProject.projectDir.parentFile
    from(sourceRoot.resolve("LICENSE"))
    from(sourceRoot.resolve("THIRD_PARTY_NOTICES.md"))
    from(sourceRoot.resolve("LICENSES")) { into("LICENSES") }
    outputDirectory.set(layout.buildDirectory.dir("generated/kogNotices"))
}
androidComponents {
    onVariants(selector().all()) { variant ->
        variant.sources.assets?.addGeneratedSourceDirectory(kogNotices, CopyKogNotices::outputDirectory)
    }
}

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
