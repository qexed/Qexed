package net.minecraft.world.level.gamerules;

import net.minecraft.util.StringRepresentable;

@net.neoforged.fml.common.asm.enumextension.NamedEnum
public enum GameRuleType implements StringRepresentable, net.neoforged.fml.common.asm.enumextension.IExtensibleEnum {
    INT("integer"),
    BOOL("boolean");

    private final String name;

    private GameRuleType(String name) {
        this.name = name;
    }

    @Override
    public String getSerializedName() {
        return this.name;
    }

    public static net.neoforged.fml.common.asm.enumextension.ExtensionInfo getExtensionInfo() {
        return net.neoforged.fml.common.asm.enumextension.ExtensionInfo.nonExtended(GameRuleType.class);
    }
}
