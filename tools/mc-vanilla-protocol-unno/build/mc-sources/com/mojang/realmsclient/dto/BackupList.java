package com.mojang.realmsclient.dto;

import com.google.gson.JsonElement;
import com.mojang.logging.LogUtils;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.util.LenientJsonParser;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.slf4j.Logger;

@OnlyIn(Dist.CLIENT)
public record BackupList(List<Backup> backups) {
    private static final Logger LOGGER = LogUtils.getLogger();

    public static BackupList parse(String json) {
        List<Backup> backups = new ArrayList<>();

        try {
            JsonElement node = LenientJsonParser.parse(json).getAsJsonObject().get("backups");
            if (node.isJsonArray()) {
                for (JsonElement element : node.getAsJsonArray()) {
                    Backup entry = Backup.parse(element);
                    if (entry != null) {
                        backups.add(entry);
                    }
                }
            }
        } catch (Exception var6) {
            LOGGER.error("Could not parse BackupList", (Throwable)var6);
        }

        return new BackupList(backups);
    }
}
