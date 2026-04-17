package com.suture.actions

import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.ui.Messages
import com.suture.services.SutureService

class SutureCommitAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val service = project.getService(SutureService::class.java)
        val baseDir = project.baseDir ?: return
        val repoRoot = service.getRepoRoot(baseDir) ?: baseDir

        val message = Messages.showInputDialog(
            project,
            "Enter commit message:",
            "Suture Commit",
            null
        ) ?: return

        if (message.isBlank()) return

        try {
            val output = service.commit(repoRoot, message)
            NotificationGroupManager.getInstance()
                .getNotificationGroup("Suture Notifications")
                .createNotification("Suture Commit", output.ifBlank { "Changes committed successfully." }, NotificationType.INFORMATION)
                .notify(project)
        } catch (e: Exception) {
            NotificationGroupManager.getInstance()
                .getNotificationGroup("Suture Notifications")
                .createNotification("Suture Error", e.message ?: "Unknown error", NotificationType.ERROR)
                .notify(project)
        }
    }
}
