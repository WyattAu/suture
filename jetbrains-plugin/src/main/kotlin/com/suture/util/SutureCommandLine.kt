package com.suture.util

import com.intellij.execution.process.ProcessOutput
import com.intellij.openapi.util.SystemInfo
import java.io.File

object SutureCommandLine {

    private fun findSutureBinary(): String? {
        val pathDirs = System.getenv("PATH")?.split(File.pathSeparator) ?: return null
        for (dir in pathDirs) {
            val binary = File(dir, if (SystemInfo.isWindows) "suture.exe" else "suture")
            if (binary.exists() && binary.canExecute()) return binary.absolutePath
        }
        val homeDir = System.getProperty("user.home")
        val commonPaths = listOf(
            "$homeDir/.cargo/bin/suture",
            "/usr/local/bin/suture",
        )
        for (path in commonPaths) {
            val f = File(path)
            if (f.exists() && f.canExecute()) return f.absolutePath
        }
        return null
    }

    fun execute(workingDir: File, vararg args: String): ProcessOutput {
        val binary = findSutureBinary()
            ?: throw RuntimeException("Suture binary not found. Install from https://github.com/WyattAu/suture")

        val process = ProcessBuilder(binary, *args)
            .directory(workingDir)
            .redirectErrorStream(true)
            .start()

        val output = process.inputStream.bufferedReader().readText()
        val exitCode = process.waitFor()

        return ProcessOutput(output, "", exitCode, false)
    }

    fun isAvailable(): Boolean = findSutureBinary() != null
}
