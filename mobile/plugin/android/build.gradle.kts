import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "io.github.micki_b.rupert.phone"
    compileSdk = 36

    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    // Custom Tabs for the OneDrive sign-in page.
    implementation("androidx.browser:browser:1.8.0")
    // Google's document scanner (edge detection, crop, cleanup; Play services).
    implementation("com.google.android.gms:play-services-mlkit-document-scanner:16.0.0")
    // Google's authorization for Drive (Android apps can't use the web sign-in).
    implementation("com.google.android.gms:play-services-auth:21.3.0")
    implementation(project(":tauri-android"))
}
