plugins {
    id("java")
    id("org.jetbrains.kotlin.jvm") version "2.1.0"
    id("org.jetbrains.intellij.platform") version "2.2.1"
}

group = "com.suture"
version = "1.0.0"

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        create("IC", "2024.2")
        bundledPlugin("com.intellij.java")
        bundledPlugin("Git4Idea")
    }
    testImplementation(kotlin("test"))
}

tasks {
    buildSearchableOptions {
        enabled = false
    }
}
