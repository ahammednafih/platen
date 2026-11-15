plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

val repo = rootDir.parentFile
val rustTarget = "aarch64-linux-android"
val jniDir = layout.buildDirectory.dir("rust/jniLibs")
val bindingsDir = layout.buildDirectory.dir("rust/kotlin")

val cargoNdk by tasks.registering(Exec::class) {
    workingDir = repo
    commandLine("mise", "exec", "--", "cargo", "ndk", "-t", "arm64-v8a", "-P", "31", "-o", jniDir.get().asFile.path, "build", "-p", "platen-mobile", "--lib", "--release")
}

val uniffiBindings by tasks.registering(Exec::class) {
    dependsOn(cargoNdk)
    doFirst { delete(bindingsDir) }
    workingDir = repo
    commandLine(
        "mise", "exec", "--", "cargo", "run", "-q", "-p", "platen-mobile", "--bin", "uniffi-bindgen", "--",
        "generate", "--no-format", "--library", "target/$rustTarget/release/libplaten_mobile.so",
        "--language", "kotlin", "--out-dir", bindingsDir.get().asFile.path,
    )
}

tasks.named("preBuild") { dependsOn(uniffiBindings) }

android {
    namespace = "com.kanamlabs.platen"
    compileSdk = 37
    ndkVersion = "30.0.16248370"
    defaultConfig {
        applicationId = "com.kanamlabs.platen"
        minSdk = 31
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
    }
    buildTypes {
        debug {
            applicationIdSuffix = ".dev"
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    buildFeatures {
        compose = true
    }
}

kotlin {
    compilerOptions.optIn.addAll("androidx.compose.material3.ExperimentalMaterial3ExpressiveApi", "androidx.compose.material3.ExperimentalMaterial3Api")
}

androidComponents {
    onVariants { variant ->
        variant.sources.jniLibs?.addStaticSourceDirectory(jniDir.get().asFile.path)
        variant.sources.kotlin?.addStaticSourceDirectory(bindingsDir.get().asFile.path)
    }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.material3)
    implementation(libs.compose.icons)
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.viewmodel)
    implementation(libs.lifecycle.runtime.compose)
    implementation("${libs.jna.get()}@aar")
}
