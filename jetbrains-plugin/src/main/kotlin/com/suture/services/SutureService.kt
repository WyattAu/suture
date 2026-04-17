package com.suture.services

import com.intellij.openapi.components.Service
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.suture.util.SutureCommandLine
import java.io.File

@Service(Service.Level.PROJECT)
class SutureService(private val project: Project) {

    fun isSutureRepo(directory: VirtualFile): Boolean {
        return File(directory.path, ".suture").isDirectory
    }

    fun init(directory: VirtualFile): String {
        val output = SutureCommandLine.execute(File(directory.path), "init", "--path", directory.path)
        if (output.exitCode != 0) throw RuntimeException(output.stdout)
        return output.stdout.trim()
    }

    fun status(directory: VirtualFile): String {
        val output = SutureCommandLine.execute(File(directory.path), "status")
        if (output.exitCode != 0) throw RuntimeException(output.stdout)
        return output.stdout
    }

    fun add(directory: VirtualFile, paths: List<String>): String {
        val args = mutableListOf("add")
        args.addAll(paths)
        val output = SutureCommandLine.execute(File(directory.path), *args.toTypedArray())
        return output.stdout.trim()
    }

    fun commit(directory: VirtualFile, message: String): String {
        val output = SutureCommandLine.execute(File(directory.path), "commit", "-m", message)
        if (output.exitCode != 0) throw RuntimeException(output.stdout)
        return output.stdout.trim()
    }

    fun push(directory: VirtualFile): String {
        val output = SutureCommandLine.execute(File(directory.path), "push")
        return if (output.exitCode == 0) output.stdout.trim() else throw RuntimeException(output.stdout)
    }

    fun pull(directory: VirtualFile): String {
        val output = SutureCommandLine.execute(File(directory.path), "pull")
        return if (output.exitCode == 0) output.stdout.trim() else throw RuntimeException(output.stdout)
    }

    fun log(directory: VirtualFile, count: Int = 20): String {
        val output = SutureCommandLine.execute(File(directory.path), "log", "--oneline", "-n", count.toString())
        return output.stdout.trim()
    }

    fun branch(directory: VirtualFile): String {
        val output = SutureCommandLine.execute(File(directory.path), "branch")
        return output.stdout.trim()
    }

    fun diff(directory: VirtualFile): String {
        val output = SutureCommandLine.execute(File(directory.path), "diff")
        return output.stdout.trim()
    }

    fun getRepoRoot(directory: VirtualFile): VirtualFile? {
        var current = directory
        while (current != null && current.isValid) {
            if (isSutureRepo(current)) return current
            current = current.parent
        }
        return null
    }
}
