plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "tokyo.runo.maidcafese"
    compileSdk = 35

    defaultConfig {
        applicationId = "tokyo.runo.maidcafese"
        minSdk = 26 // java.time を desugaring 無しで使うため
        targetSdk = 35
        versionCode = 5
        versionName = "0.4.0"
    }

    // 正式リリース署名。鍵情報は環境変数からのみ受け取り、このファイルにも他のファイルにも秘密を書かない
    // (keystore本体は絶対にコミットしない。`installer/build-release.ps1`が環境変数を設定してビルドする)。
    // 環境変数が揃っていない場合、releaseビルドは署名なしのまま(デバッグ署名のフローには影響しない)。
    val releaseStoreFile = System.getenv("MAID_CAFE_SE_KEYSTORE_FILE")
    val releaseStorePassword = System.getenv("MAID_CAFE_SE_KEYSTORE_PASSWORD")
    val releaseKeyAlias = System.getenv("MAID_CAFE_SE_KEY_ALIAS")
    val releaseKeyPassword = System.getenv("MAID_CAFE_SE_KEY_PASSWORD")
    val hasReleaseSigning = !releaseStoreFile.isNullOrBlank() && !releaseStorePassword.isNullOrBlank() &&
        !releaseKeyAlias.isNullOrBlank() && !releaseKeyPassword.isNullOrBlank()

    signingConfigs {
        if (hasReleaseSigning) {
            create("release") {
                storeFile = file(releaseStoreFile!!)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (hasReleaseSigning) signingConfig = signingConfigs.getByName("release")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
    // Android版とWindows版(desktop)で共有する画面部品
    sourceSets.getByName("main").java.srcDir("../shared-ui/src")
}

dependencies {
    implementation(project(":core"))
    val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
    implementation(composeBom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.core:core-ktx:1.13.1")
}
