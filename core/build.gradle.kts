plugins {
    id("org.jetbrains.kotlin.jvm")
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    testImplementation(kotlin("test"))
}

// テストは、ホスト向けにビルドしたRust製コア(JNI)を読み込んで、本物のJNI経由で動かす
val osName = System.getProperty("os.name").lowercase()
val nativeFile = when {
    "win" in osName -> "maid_cafe_jni.dll"
    "mac" in osName -> "libmaid_cafe_jni.dylib"
    else -> "libmaid_cafe_jni.so"
}
val cargoExe = File(System.getProperty("user.home"), ".cargo/bin/" + if ("win" in osName) "cargo.exe" else "cargo")

val buildHostNative = tasks.register<Exec>("buildHostNative") {
    group = "build"
    description = "テスト用に、Rust製コア(crates/maid-cafe-jni)をこのPC向けにビルドする"
    workingDir = rootDir
    commandLine(if (cargoExe.exists()) cargoExe.absolutePath else "cargo", "build", "-p", "maid-cafe-jni")
}

tasks.test {
    useJUnitPlatform()
    dependsOn(buildHostNative)
    systemProperty("maidcafe.native", File(rootDir, "target/debug/$nativeFile").absolutePath)
}
