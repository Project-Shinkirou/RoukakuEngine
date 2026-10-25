import java.util.Properties
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.TaskAction

abstract class VerifyRoukakuAndroidInputs : DefaultTask() {
    @get:InputDirectory
    abstract val rustJniDir: DirectoryProperty

    @get:InputFile
    abstract val bundledPackage: RegularFileProperty

    @TaskAction
    fun verify() {
        require(bundledPackage.get().asFile.extension == "roukakupkg") {
            "ROUKAKU_ANDROID_PACKAGE_INVALID: bundled package must use the .roukakupkg extension"
        }
        require(rustJniDir.file("arm64-v8a/libroukaku_player_android.so").get().asFile.isFile) {
            "ROUKAKU_ANDROID_NATIVE_LIBRARY_REQUIRED: arm64-v8a cdylib is missing"
        }
    }
}

plugins {
    id("com.android.application")
}

val rustJniDirectoryPath = providers.gradleProperty("roukakuRustJniDir")
    .orElse(layout.projectDirectory.dir("generated/jniLibs").asFile.absolutePath)
val bundledPackagePath = providers.gradleProperty("roukakuBundledPackage")
val externalSigning = providers.gradleProperty("roukakuSigningProperties")

android {
    namespace = "org.shinkirou.roukakuplayer"
    compileSdk {
        version = release(37) {
            minorApiLevel = 2
        }
    }
    buildToolsVersion = "37.0.0"
    ndkVersion = "30.0.16248370"

    defaultConfig {
        applicationId = providers.gradleProperty("roukakuApplicationId").orElse("org.shinkirou.roukakuplayer").get()
        minSdk = 28
        targetSdk = 37
        versionCode = providers.gradleProperty("roukakuVersionCode").orElse("1").get().toInt()
        versionName = providers.gradleProperty("roukakuVersionName").orElse("0.1.0").get()
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    sourceSets.named("main") {
        jniLibs.directories.apply {
            clear()
            add(rustJniDirectoryPath.get())
        }
        assets.directories.apply {
            clear()
            add(layout.buildDirectory.dir("generated/roukakuAssets").get().asFile.absolutePath)
        }
    }

    androidResources {
        noCompress += "roukakupkg"
    }

    buildFeatures {
        buildConfig = false
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }

    signingConfigs {
        if (externalSigning.isPresent) {
            create("externalRelease") {
                val properties = Properties().apply {
                    externalSigning.get().let { file(it).inputStream().use(::load) }
                }
                storeFile = file(requireNotNull(properties.getProperty("storeFile")))
                storePassword = requireNotNull(properties.getProperty("storePassword"))
                keyAlias = requireNotNull(properties.getProperty("keyAlias"))
                keyPassword = requireNotNull(properties.getProperty("keyPassword"))
            }
        }
    }

    buildTypes {
        debug {
            isJniDebuggable = true
        }
        release {
            isMinifyEnabled = false
            isDebuggable = false
            signingConfig = signingConfigs.findByName("externalRelease")
        }
    }
}

val prepareRoukakuAssets = tasks.register<Copy>("prepareRoukakuAssets") {
    bundledPackagePath.orNull?.let { source ->
        from(source)
        into(layout.buildDirectory.dir("generated/roukakuAssets"))
        rename { "game.roukakupkg" }
    }
}

val verifyRoukakuAndroidInputs = tasks.register<VerifyRoukakuAndroidInputs>("verifyRoukakuAndroidInputs") {
    rustJniDir.set(layout.dir(rustJniDirectoryPath.map(::file)))
    bundledPackage.set(layout.file(bundledPackagePath.map(::file)))
}

tasks.named("preBuild").configure {
    dependsOn(prepareRoukakuAssets, verifyRoukakuAndroidInputs)
}

dependencies {
    implementation("androidx.activity:activity-ktx:1.13.0")
    implementation("androidx.appcompat:appcompat:1.8.0")
    implementation("androidx.games:games-activity:4.4.2")
}
