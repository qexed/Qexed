package com.mojang.blaze3d.systems;

import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public class BackendCreationException extends Exception {
    public BackendCreationException(String message) {
        super(message);
    }
}
