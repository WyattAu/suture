package com.suture.actions

import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.ui.Messages
import com.suture.services.SutureService

class SutureAddAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val service = project.getService(SutureService::class.java)
        val baseDir = project.baseDir ?: return
        val repoRoot = service.getRepoRoot(baseDir) ?: baseDir

        val paths = Messages.showInputDialog(
            project,
            "Enter file paths to add (comma-separated):",
            "Suture Add",
            null
        ) ?: return

        try {
            val fileList = paths.split(",").map { it.trim() }.filter { it.isNotBlank() }
            if (fileList.isEmpty()) return
            val output = service.add(repoRoot, fileList)
            NotificationGroupManager.getInstance()
                .getNotificationGroup("Suture Notifications")
                .createNotification("Suture Add", output.ifBlank { "Files added successfully." }, NotificationType.INFORMATION)
                .notify(project)
        } catch (e: Exception) {
            NotificationGroupManager.getInstance()
                .getNotificationGroup("Suture Notifications")
                .createNotification("Suture Error", e.message ?: "Unknown error", NotificationType.ERROR)
                .notify(project)
        }
    }
}
