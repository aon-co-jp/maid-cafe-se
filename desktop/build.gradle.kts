// Windows版(デスクトップ)。core(繰り返しルール・祝日・Planner・音声処理)とshared-ui(画面部品)をAndroid版と共有する。
plugins {
    id("org.jetbrains.kotlin.jvm")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
}

kotlin {
    jvmToolchain(17)
}

sourceSets.getByName("main") {
    // Android版と共有する画面部品と、Android版と同じ生成音源(chime/alarm/melody.wav)
    kotlin.srcDir("../shared-ui/src")
    resources.srcDir("../app/src/main/res/raw")
}

dependencies {
    implementation(project(":core"))
    implementation(compose.desktop.currentOs)
    implementation(compose.material3)
    testImplementation(kotlin("test"))
}

tasks.test {
    useJUnitPlatform()
    // DesktopStoreの保存先をビルド配下の使い捨てフォルダにする(実際の%APPDATA%を汚さない)
    systemProperty("maidcafese.data", layout.buildDirectory.dir("test-data").get().asFile.also { it.mkdirs() }.absolutePath)
}

compose.desktop {
    application {
        mainClass = "tokyo.runo.maidcafese.desktop.MainKt"
        // jpackage(実行環境同梱)にはjpackageを含むJDKが必要。Gradle自体は別のJDKで動いていてよい。
        javaHome = System.getenv("MAID_CAFE_SE_JPACKAGE_JDK") ?: "C:/Program Files/Java/jdk-17"
        nativeDistributions {
            packageName = "maid-cafe-se"
            packageVersion = "0.4.0"
            // jpackageの引数ファイルはWindowsの既定文字コード(MS932)で読まれるので、ここは非ASCII文字を使わない
            description = "maid-cafe-se: voice alarm secretary (maid cafe style)"
            vendor = "aon-co-jp"
            // 使うJavaモジュールだけに絞って実行環境を小さくする
            modules("java.desktop", "java.logging", "java.management", "jdk.unsupported")
        }
    }
}
