package com.suture;

import com.intellij.openapi.project.Project;
import com.intellij.openapi.vcs.AbstractVcs;
import com.intellij.openapi.vcs.VcsConfiguration;
import com.intellij.openapi.vcs.VcsKey;
import com.intellij.openapi.vcs.VcsShowConfirmationOption;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

public class SutureVcs extends AbstractVcs {
    public static final String NAME = "Suture";
    public static final VcsKey KEY = createKey(NAME);

    public SutureVcs(@NotNull Project project) {
        super(project);
    }

    @Override
    @NotNull
    public String getDisplayName() {
        return NAME;
    }

    @Override
    @NotNull
    public VcsKey getKey() {
        return KEY;
    }

    @Override
    @Nullable
    public VcsShowConfirmationOption getConfirmAddAction() {
        return VcsConfiguration.StandardConfirmation.ADD;
    }
}
