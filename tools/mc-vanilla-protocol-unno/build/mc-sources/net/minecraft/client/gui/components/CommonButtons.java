package net.minecraft.client.gui.components;

import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public class CommonButtons {
    public static SpriteIconButton language(int width, Button.OnPress onPress, boolean iconOnly) {
        return SpriteIconButton.builder(Component.translatable("options.language"), onPress, iconOnly)
            .width(width)
            .sprite(Identifier.withDefaultNamespace("icon/language"), 15, 15)
            .build();
    }

    public static SpriteIconButton accessibility(int width, Button.OnPress onPress, boolean iconOnly) {
        Component text = iconOnly ? Component.translatable("options.accessibility") : Component.translatable("accessibility.onboarding.accessibility.button");
        return SpriteIconButton.builder(text, onPress, iconOnly).width(width).sprite(Identifier.withDefaultNamespace("icon/accessibility"), 15, 15).build();
    }
}
