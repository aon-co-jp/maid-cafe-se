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
        versionCode = 9
        versionName = "0.5.3"
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
    // Android版の画面部品(以前はWindows版のKotlin版と共有していた)
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

// Rust製コア(crates/maid-cafe-jni)を cargo-ndk で各ABIの.soにして、APKに入れる。
// 要: Rust(+ `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`)、`cargo install cargo-ndk`、Android NDK。
val rustJniLibs = layout.projectDirectory.dir("src/main/jniLibs")
val buildRustJni = tasks.register<Exec>("buildRustJni") {
    group = "build"
    description = "Rust製コアを cargo-ndk でビルドして src/main/jniLibs へ出力する"
    workingDir = rootDir
    val home = File(System.getProperty("user.home"))
    val isWin = "win" in System.getProperty("os.name").lowercase()
    val cargoBin = File(home, ".cargo/bin")
    val sdkDir = (File(rootDir, "local.properties").takeIf { it.exists() }?.readLines()
        ?.firstOrNull { it.startsWith("sdk.dir=") }?.substringAfter("=")?.replace("\\\\", "/")
        ?: System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT"))
    val ndk = System.getenv("ANDROID_NDK_HOME")
        ?: sdkDir?.let { File(it, "ndk").listFiles()?.filter { f -> f.isDirectory }?.maxByOrNull { f -> f.name }?.absolutePath }
    environment("PATH", cargoBin.absolutePath + File.pathSeparator + System.getenv("PATH"))
    if (ndk != null) environment("ANDROID_NDK_HOME", ndk)
    commandLine(
        File(cargoBin, if (isWin) "cargo.exe" else "cargo").absolutePath, "ndk",
        "-t", "arm64-v8a", "-t", "armeabi-v7a", "-t", "x86_64", "-P", "26",
        "-o", rustJniLibs.asFile.absolutePath, "build", "--release", "-p", "maid-cafe-jni",
    )
}
tasks.matching { it.name == "preBuild" }.configureEach { dependsOn(buildRustJni) }
