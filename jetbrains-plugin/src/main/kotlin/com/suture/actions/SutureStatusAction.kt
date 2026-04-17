package com.suture.actions

import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.suture.services.SutureService

class SutureStatusAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val service = project.getService(SutureService::class.java)
        val baseDir = project.baseDir ?: return

        try {
            val output = service.status(baseDir)
            NotificationGroupManager.getInstance()
                .getNotificationGroup("Suture Notifications")
                .createNotification("Suture Status", output, NotificationType.INFORMATION)
                .notify(project)
        } catch (e: Exception) {
            NotificationGroupManager.getInstance()
                .getNotificationGroup("Suture Notifications")
                .createNotification("Suture Error", e.message ?: "Unknown error", NotificationType.ERROR)
                .notify(project)
        }
    }
}
