plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.readest.nas_smb"
    compileSdk = 36

    defaultConfig {
        // smbj necesita APIs de Java 8+; 26 coincide además con el minSdk de la app.
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
}

dependencies {
    // Cliente SMB/CIFS: el mismo que ya usaba NasConnect contra tu TrueNAS.
    implementation("com.hierynomus:smbj:0.13.0")
    // smbj registra sus trazas con slf4j; sin binder se queda en no-op, que es
    // justo lo que queremos en la app.
    implementation("org.slf4j:slf4j-api:1.7.36")
    implementation("androidx.core:core-ktx:1.12.0")
    implementation(project(":tauri-android"))
}
