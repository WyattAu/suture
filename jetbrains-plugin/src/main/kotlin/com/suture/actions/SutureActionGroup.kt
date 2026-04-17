package com.suture.actions

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.suture.util.SutureCommandLine

class SutureActionGroup : AnAction() {
    override fun update(e: AnActionEvent) {
        e.presentation.isEnabledAndVisible = SutureCommandLine.isAvailable()
    }
}
