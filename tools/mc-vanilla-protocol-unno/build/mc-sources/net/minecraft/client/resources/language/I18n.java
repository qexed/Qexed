package net.minecraft.client.resources.language;

import java.util.IllegalFormatException;
import java.util.Locale;
import net.minecraft.locale.Language;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public class I18n {
    private static volatile Language language = Language.getInstance();

    private I18n() {
    }

    static void setLanguage(Language locale) {
        language = locale;
        net.neoforged.fml.i18n.I18nManager.injectTranslations(locale.getLanguageData());
    }

    public static String get(String id, Object... args) {
        String value = language.getOrDefault(id);

        try {
            return String.format(Locale.ROOT, value, args);
        } catch (IllegalFormatException var4) {
            return "Format error: " + value;
        }
    }

    public static boolean exists(String id) {
        return language.has(id);
    }
}
