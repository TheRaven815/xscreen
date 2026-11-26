#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Tr,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Tr => "tr",
        }
    }

    pub fn from_code(s: &str) -> Self {
        if s.eq_ignore_ascii_case("tr") {
            Self::Tr
        } else {
            Self::En
        }
    }
}

pub struct Strings {
    pub lang: Lang,
}

impl Strings {
    pub const fn new(lang: Lang) -> Self {
        Self { lang }
    }

    #[inline]
    fn pick(&self, en: &'static str, tr: &'static str) -> &'static str {
        match self.lang {
            Lang::En => en,
            Lang::Tr => tr,
        }
    }

    // Window & Navigation
    pub fn tab_album(&self) -> &'static str {
        self.pick("Album", "Albüm")
    }
    pub fn tab_editor(&self) -> &'static str {
        self.pick("Editor", "Editör")
    }
    pub fn tab_settings(&self) -> &'static str {
        self.pick("Settings", "Ayarlar")
    }
    pub fn new_capture(&self) -> &'static str {
        self.pick("Capture", "Yakala")
    }
    pub fn new_capture_tip(&self) -> &'static str {
        self.pick("Start screen capture", "Ekran yakalamayı başlat")
    }
    pub fn theme_dark(&self) -> &'static str {
        self.pick("Dark", "Koyu")
    }
    pub fn theme_light(&self) -> &'static str {
        self.pick("Light", "Açık")
    }
    pub fn switch_theme_tip(&self) -> &'static str {
        self.pick("Switch dark / light theme", "Koyu / açık temayı değiştir")
    }
    pub fn dismiss(&self) -> &'static str {
        self.pick("Dismiss", "Kapat")
    }

    // Album
    pub fn search_hint(&self) -> &'static str {
        self.pick("Filter files…", "Dosya ara…")
    }
    pub fn clear_search(&self) -> &'static str {
        self.pick("Clear filter", "Filtreyi temizle")
    }
    pub fn refresh(&self) -> &'static str {
        self.pick("Refresh", "Yenile")
    }
    pub fn open_folder(&self) -> &'static str {
        self.pick("Folder", "Klasör")
    }
    pub fn open_folder_tip(&self) -> &'static str {
        self.pick(
            "Open capture folder in Explorer",
            "Kayıt klasörünü Gezgin'de aç",
        )
    }
    pub fn loading_captures(&self) -> &'static str {
        self.pick("Scanning capture folder…", "Kayıt klasörü taranıyor…")
    }
    pub fn empty_album_title(&self) -> &'static str {
        self.pick(
            "No saved captures yet",
            "Henüz kaydedilmiş ekran görüntüsü yok",
        )
    }
    pub fn empty_album_sub(&self) -> &'static str {
        self.pick(
            "Saved screenshots appear here automatically. Click any capture to annotate or crop it in the Editor.",
            "Kaydedilen ekran görüntüleri burada listelenir. Herhangi bir görsele tıklayarak Editör'de düzenleyebilirsiniz.",
        )
    }
    pub fn no_matches(&self) -> &'static str {
        self.pick("No matching files", "Eşleşen dosya bulunamadı")
    }
    pub fn edit_image(&self) -> &'static str {
        self.pick("Edit", "Düzenle")
    }
    pub fn copy_image(&self) -> &'static str {
        self.pick("Copy", "Kopyala")
    }
    pub fn open_external(&self) -> &'static str {
        self.pick("Open externally", "Dışarıda aç")
    }
    pub fn delete(&self) -> &'static str {
        self.pick("Delete", "Sil")
    }
    pub fn confirm_delete(&self) -> &'static str {
        self.pick("Confirm delete", "Silmeyi onayla")
    }
    pub fn cancel(&self) -> &'static str {
        self.pick("Cancel", "İptal")
    }
    pub fn loading_thumb(&self) -> &'static str {
        self.pick("Loading…", "Yükleniyor…")
    }
    pub fn preview_unavailable(&self) -> &'static str {
        self.pick("Unavailable", "Okunamadı")
    }
    pub fn copied_notice(&self) -> &'static str {
        self.pick("Copied to clipboard", "Panoya kopyalandı")
    }
    pub fn saved_notice(&self) -> &'static str {
        self.pick("Saved changes", "Değişiklikler kaydedildi")
    }
    pub fn saved_copy_notice(&self) -> &'static str {
        self.pick("Saved as new copy", "Yeni kopya olarak kaydedildi")
    }
    pub fn deleted_notice(&self) -> &'static str {
        self.pick("Capture deleted", "Görsel silindi")
    }
    pub fn status_tray_hint(&self) -> &'static str {
        self.pick(
            "Runs in system tray when closed",
            "Pencere kapatıldığında sistem tepsisinde çalışmaya devam eder",
        )
    }

    // Editor
    pub fn editor_empty_title(&self) -> &'static str {
        self.pick("No image loaded in Editor", "Editör'de açık görsel yok")
    }
    pub fn editor_empty_sub(&self) -> &'static str {
        self.pick(
            "Select a capture from Album, paste an image from clipboard (Ctrl+V), or start a new screen capture.",
            "Albüm'den bir görsel seçin, panodan yapıştırın (Ctrl+V) veya yeni ekran görüntüsü yakalayın.",
        )
    }
    pub fn paste_clipboard(&self) -> &'static str {
        self.pick("Paste clipboard (Ctrl+V)", "Panodan yapıştır (Ctrl+V)")
    }
    pub fn paste_short(&self) -> &'static str {
        self.pick("Paste", "Yapıştır")
    }
    pub fn browse_album(&self) -> &'static str {
        self.pick("Open from Album", "Albüm'den seç")
    }
    pub fn save(&self) -> &'static str {
        self.pick("Save", "Kaydet")
    }
    pub fn save_tip(&self) -> &'static str {
        self.pick("Save (Ctrl+S)", "Kaydet (Ctrl+S)")
    }
    pub fn save_copy(&self) -> &'static str {
        self.pick("Save copy", "Kopya kaydet")
    }
    pub fn save_copy_tip(&self) -> &'static str {
        self.pick(
            "Save as a new PNG file in capture folder (Ctrl+Shift+S)",
            "Kayıt klasörüne yeni PNG olarak kaydet (Ctrl+Shift+S)",
        )
    }
    pub fn copy_tip(&self) -> &'static str {
        self.pick(
            "Copy edited image to clipboard (Ctrl+C)",
            "Düzenlenen görseli panoya kopyala (Ctrl+C)",
        )
    }
    pub fn clear_annotations_tip(&self) -> &'static str {
        self.pick("Remove all annotations", "Tüm çizimleri temizle")
    }
    pub fn undo_tip(&self) -> &'static str {
        self.pick("Undo (Ctrl+Z)", "Geri al (Ctrl+Z)")
    }
    pub fn redo_tip(&self) -> &'static str {
        self.pick("Redo (Ctrl+Y)", "İleri al (Ctrl+Y)")
    }
    pub fn fit_view(&self) -> &'static str {
        self.pick("Fit", "Sığdır")
    }
    pub fn unsaved_badge(&self) -> &'static str {
        self.pick("Modified", "Değiştirildi")
    }
    pub fn unsaved_changes(&self) -> &'static str {
        self.pick("Unsaved changes", "Kaydedilmemiş değişiklikler")
    }
    pub fn saving_image(&self) -> &'static str {
        self.pick("Saving image…", "Görsel kaydediliyor…")
    }
    pub fn discard_to_album(&self) -> &'static str {
        self.pick(
            "Discard your changes and return to the Album?",
            "Değişiklikleri kaydetmeden Albüm’e dönmek istiyor musunuz?",
        )
    }
    pub fn discard_to_replace(&self) -> &'static str {
        self.pick(
            "Replace this image without saving your changes?",
            "Değişiklikleri kaydetmeden bu görseli değiştirmek istiyor musunuz?",
        )
    }
    pub fn yes(&self) -> &'static str {
        self.pick("Yes", "Evet")
    }
    pub fn no(&self) -> &'static str {
        self.pick("No", "Hayır")
    }
    pub fn clipboard_source(&self) -> &'static str {
        self.pick("Clipboard image", "Pano görseli")
    }
    pub fn new_capture_source(&self) -> &'static str {
        self.pick("New capture", "Yeni ekran görüntüsü")
    }

    // Tools & Properties
    pub fn tool_select(&self) -> &'static str {
        self.pick("Select / Move (V)", "Seç / Düzenle (V)")
    }
    pub fn tool_arrow(&self) -> &'static str {
        self.pick("Arrow (A)", "Ok (A)")
    }
    pub fn tool_line(&self) -> &'static str {
        self.pick("Line (L)", "Çizgi (L)")
    }
    pub fn tool_rect(&self) -> &'static str {
        self.pick("Rectangle (R)", "Dikdörtgen (R)")
    }
    pub fn tool_ellipse(&self) -> &'static str {
        self.pick("Ellipse (E)", "Elips (E)")
    }
    pub fn tool_triangle(&self) -> &'static str {
        self.pick("Triangle", "Üçgen")
    }
    pub fn tool_pen(&self) -> &'static str {
        self.pick("Pen (P)", "Kalem (P)")
    }
    pub fn tool_marker(&self) -> &'static str {
        self.pick("Highlighter (H)", "Fosforlu Kalem (H)")
    }
    pub fn tool_text(&self) -> &'static str {
        self.pick("Text (T)", "Metin (T)")
    }
    pub fn tool_number(&self) -> &'static str {
        self.pick("Step Number (N)", "Adım Numarası (N)")
    }
    pub fn tool_blur(&self) -> &'static str {
        self.pick("Blur / Mosaic (B)", "Bulanıklaştır / Mozaik (B)")
    }
    pub fn tool_crop(&self) -> &'static str {
        self.pick("Crop (C)", "Kırp (C)")
    }
    pub fn blur_soft(&self) -> &'static str {
        self.pick("Soft blur", "Yumuşak bulanıklık")
    }
    pub fn blur_mosaic(&self) -> &'static str {
        self.pick("Mosaic", "Mozaik (Piksel)")
    }
    pub fn blur_heavy(&self) -> &'static str {
        self.pick("Heavy blur", "Yoğun bulanıklık")
    }
    pub fn prop_stroke(&self) -> &'static str {
        self.pick("Size", "Boyut")
    }
    pub fn prop_text_size(&self) -> &'static str {
        self.pick("Font", "Punto")
    }
    pub fn prop_fill(&self) -> &'static str {
        self.pick("Fill", "Dolgu")
    }
    pub fn prop_outline(&self) -> &'static str {
        self.pick("Outline", "Çizgi")
    }
    pub fn prop_selected(&self) -> &'static str {
        self.pick("Selected:", "Seçili:")
    }
    pub fn prop_duplicate(&self) -> &'static str {
        self.pick("Duplicate", "Çoğalt")
    }
    pub fn prop_front(&self) -> &'static str {
        self.pick("To front", "Öne getir")
    }
    pub fn prop_delete_item(&self) -> &'static str {
        self.pick("Delete (Del)", "Sil (Del)")
    }
    pub fn prop_select_hint(&self) -> &'static str {
        self.pick(
            "Click any drawn item to move, resize handles, recolor, or press Delete",
            "Düzenlemek, taşımak, boyutlandırmak veya silmek için çizilmiş bir elemana tıklayın",
        )
    }
    pub fn crop_apply(&self) -> &'static str {
        self.pick("Apply crop", "Kırpmayı uygula")
    }
    pub fn crop_hint(&self) -> &'static str {
        self.pick(
            "Drag on image to select crop area",
            "Kırpılacak alanı seçmek için görsel üzerinde sürükleyin",
        )
    }
    pub fn text_placeholder(&self) -> &'static str {
        self.pick("Type text…", "Metin yazın…")
    }
    pub fn color_tip(&self) -> &'static str {
        self.pick("Color", "Renk")
    }
    pub fn window_pick_badge(&self, w: u32, h: u32) -> String {
        match self.lang {
            Lang::En => format!("Window · {w} × {h}  (Click)"),
            Lang::Tr => format!("Pencere · {w} × {h}  (Tıkla)"),
        }
    }

    // Time & Error messages
    pub fn time_now(&self) -> &'static str {
        self.pick("now", "şimdi")
    }
    pub fn time_mins_ago(&self, m: u64) -> String {
        match self.lang {
            Lang::En => format!("{m}m ago"),
            Lang::Tr => format!("{m}dk önce"),
        }
    }
    pub fn time_hours_ago(&self, h: u64) -> String {
        match self.lang {
            Lang::En => format!("{h}h ago"),
            Lang::Tr => format!("{h}sa önce"),
        }
    }
    pub fn time_days_ago(&self, d: u64) -> String {
        match self.lang {
            Lang::En => format!("{d}d ago"),
            Lang::Tr => format!("{d}g önce"),
        }
    }
    pub fn time_months_ago(&self, mo: u64) -> String {
        match self.lang {
            Lang::En => format!("{mo}mo ago"),
            Lang::Tr => format!("{mo}ay önce"),
        }
    }
    pub fn err_clipboard_busy(&self) -> &'static str {
        self.pick("Clipboard is busy.", "Pano şu anda meşgul.")
    }
    pub fn err_clipboard_unsupported(&self) -> &'static str {
        self.pick(
            "Unsupported clipboard bitmap.",
            "Desteklenmeyen pano görseli.",
        )
    }
    pub fn err_clipboard_no_image(&self) -> &'static str {
        self.pick(
            "Clipboard does not contain an image.",
            "Panoda görsel bulunamadı.",
        )
    }
    pub fn err_copy_failed(&self) -> &'static str {
        self.pick(
            "Could not copy image to clipboard.",
            "Görsel panoya kopyalanamadı.",
        )
    }
    pub fn err_save_settings(&self) -> &'static str {
        self.pick("Could not save settings.", "Ayarlar kaydedilemedi.")
    }
    pub fn err_startup_failed(&self) -> &'static str {
        self.pick(
            "Could not update startup setting.",
            "Başlangıç ayarı güncellenemedi.",
        )
    }
    pub fn err_delete_file(&self, e: &dyn std::fmt::Display) -> String {
        match self.lang {
            Lang::En => format!("Could not delete file: {e}"),
            Lang::Tr => format!("Dosya silinemedi: {e}"),
        }
    }
    pub fn err_save_file(&self, e: &dyn std::fmt::Display) -> String {
        match self.lang {
            Lang::En => format!("Could not save file: {e}"),
            Lang::Tr => format!("Dosya kaydedilemedi: {e}"),
        }
    }
    pub fn err_read_folder(&self) -> &'static str {
        self.pick(
            "Could not read capture folder. Check path and permissions in Settings, then Refresh.",
            "Kayıt klasörü okunamadı. Ayarlar'dan yolu ve izinleri kontrol edip Yenile'ye tıklayın.",
        )
    }

    // Settings
    pub fn sec_general(&self) -> &'static str {
        self.pick("General", "Genel")
    }
    pub fn sec_capture(&self) -> &'static str {
        self.pick("Capture & Shortcut", "Yakalama ve Kısayol")
    }
    pub fn sec_storage(&self) -> &'static str {
        self.pick("Storage & Clipboard", "Depolama ve Pano")
    }
    pub fn sec_updates(&self) -> &'static str {
        self.pick("Updates", "Güncellemeler")
    }
    pub fn update_installed(&self, version: &str) -> String {
        format!("{} v{version}", self.pick("Installed:", "Yüklü sürüm:"))
    }
    pub fn update_check(&self) -> &'static str {
        self.pick("Check for updates", "Güncellemeleri denetle")
    }
    pub fn update_checking(&self) -> &'static str {
        self.pick("Checking GitHub…", "GitHub denetleniyor…")
    }
    pub fn update_current(&self) -> &'static str {
        self.pick("xscreen is up to date.", "xscreen güncel.")
    }
    pub fn update_no_release(&self) -> &'static str {
        self.pick(
            "No stable release on GitHub yet.",
            "GitHub’da henüz kararlı sürüm yok.",
        )
    }
    pub fn update_available(&self, tag: &str) -> String {
        match self.lang {
            Lang::En => format!("{tag} is available."),
            Lang::Tr => format!("{tag} sürümü hazır."),
        }
    }
    pub fn update_notes(&self) -> &'static str {
        self.pick("Release notes", "Sürüm notları")
    }
    pub fn update_releases(&self) -> &'static str {
        self.pick("Open GitHub Releases", "GitHub sürümlerini aç")
    }
    pub fn update_download(&self) -> &'static str {
        self.pick("Download update", "Güncellemeyi indir")
    }
    pub fn update_downloading(&self, tag: &str) -> String {
        match self.lang {
            Lang::En => format!("Downloading and verifying {tag}…"),
            Lang::Tr => format!("{tag} indiriliyor ve doğrulanıyor…"),
        }
    }
    pub fn update_ready(&self, tag: &str) -> String {
        match self.lang {
            Lang::En => format!("{tag} verified. Install to restart xscreen."),
            Lang::Tr => format!("{tag} doğrulandı. Kurulum xscreen’i yeniden başlatır."),
        }
    }
    pub fn update_install(&self) -> &'static str {
        self.pick("Install and restart", "Kur ve yeniden başlat")
    }
    pub fn update_blocked(&self) -> &'static str {
        self.pick(
            "Save or discard Editor changes and finish any active capture or edit before installing.",
            "Kurulumdan önce Editör değişikliklerini kaydet veya at; açık yakalama ya da düzenlemeyi bitir.",
        )
    }
    pub fn update_failure(&self, failure: crate::update::Failure) -> &'static str {
        use crate::update::Failure;
        match failure {
            Failure::Network => self.pick(
                "Could not reach GitHub. Check your connection and try again.",
                "GitHub’a ulaşılamadı. Bağlantını kontrol edip tekrar dene.",
            ),
            Failure::RateLimited => self.pick(
                "GitHub refused the request. Try again later.",
                "GitHub isteği reddetti. Daha sonra tekrar dene.",
            ),
            Failure::Release => self.pick(
                "No usable Windows x64 package found. Check GitHub Releases.",
                "Kullanılabilir Windows x64 paketi bulunamadı. GitHub sürümlerini kontrol et.",
            ),
            Failure::Integrity => self.pick(
                "Verification failed. No files were replaced. Try downloading again.",
                "Doğrulama başarısız. Uygulama dosyası değiştirilmedi. Tekrar indirmeyi dene.",
            ),
            Failure::Storage => self.pick(
                "Could not stage the update. Check free space and write access to the xscreen folder.",
                "Güncelleme hazırlanamadı. Boş alanı ve xscreen klasörünün yazma iznini kontrol et.",
            ),
            Failure::Worker => self.pick(
                "The update task stopped. Try again.",
                "Güncelleme işlemi durdu. Tekrar dene.",
            ),
        }
    }
    pub fn update_install_failed(&self, asset: &str) -> String {
        match self.lang {
            Lang::En => format!(
                "Could not finish the update. You can download {asset} from GitHub Releases and replace the old file after closing xscreen."
            ),
            Lang::Tr => format!(
                "Güncelleme tamamlanamadı. GitHub sürümlerinden {asset} dosyasını indirip xscreen kapalıyken eski dosyanın yerine koyabilirsin."
            ),
        }
    }
    pub fn update_backup_path(&self, path: &str) -> String {
        match self.lang {
            Lang::En => format!(
                "\n\nThe previous EXE could not be restored automatically. A backup is at:\n{path}"
            ),
            Lang::Tr => format!("\n\nEski EXE otomatik geri yüklenemedi. Yedek dosya:\n{path}"),
        }
    }
    pub fn lbl_language(&self) -> &'static str {
        self.pick("Interface language", "Arayüz dili")
    }
    pub fn desc_language(&self) -> &'static str {
        self.pick(
            "Applies to main window, editor, capture toolbar, and tray menu",
            "Ana pencere, editör, yakalama çubuğu ve tepsi menüsüne uygulanır",
        )
    }
    pub fn lbl_theme(&self) -> &'static str {
        self.pick("Appearance theme", "Görünüm teması")
    }
    pub fn desc_theme(&self) -> &'static str {
        self.pick(
            "Synchronized across studio, capture toolbar, and system tray",
            "Stüdyo, yakalama araç çubuğu ve sistem tepsisi genelinde eşzamanlıdır",
        )
    }
    pub fn lbl_startup(&self) -> &'static str {
        self.pick("Launch at Windows sign-in", "Windows açılışında başlat")
    }
    pub fn desc_startup(&self) -> &'static str {
        self.pick(
            "Start silently in the system tray when you log in",
            "Oturum açıldığında sistem tepsisinde hazır bekler",
        )
    }
    pub fn lbl_hotkey(&self) -> &'static str {
        self.pick("Global capture shortcut", "Genel yakalama kısayolu")
    }
    pub fn apply(&self) -> &'static str {
        self.pick("Apply", "Uygula")
    }
    pub fn applying(&self) -> &'static str {
        self.pick("Applying…", "Uygulanıyor…")
    }
    pub fn active_prefix(&self) -> &'static str {
        self.pick("Active:", "Aktif:")
    }
    pub fn none(&self) -> &'static str {
        self.pick("Unavailable", "Yok")
    }
    pub fn lbl_cursor(&self) -> &'static str {
        self.pick("Include mouse cursor", "Fare imlecini dahil et")
    }
    pub fn desc_cursor(&self) -> &'static str {
        self.pick(
            "Composite the active cursor icon onto captured screenshots",
            "Yakalama anındaki fare imlecini ekran görüntüsüne işler",
        )
    }
    pub fn lbl_magnifier(&self) -> &'static str {
        self.pick("Show pixel magnifier", "Seçim büyütecini göster")
    }
    pub fn desc_magnifier(&self) -> &'static str {
        self.pick(
            "Show zoomed pixel grid and HEX color next to the cursor",
            "Alan seçerken imleç yanında piksel ızgarası ve HEX kodu gösterir",
        )
    }
    pub fn lbl_save_folder(&self) -> &'static str {
        self.pick("Capture folder", "Kayıt klasörü")
    }
    pub fn desc_save_folder(&self) -> &'static str {
        self.pick(
            "Leave empty to use default Pictures\\xscreen directory",
            "Boş bırakıldığında varsayılan Pictures\\xscreen klasörü kullanılır",
        )
    }
    pub fn lbl_copy_png(&self) -> &'static str {
        self.pick(
            "Write PNG format to clipboard",
            "Panoya PNG formatında da yaz",
        )
    }
    pub fn desc_copy_png(&self) -> &'static str {
        self.pick(
            "Keeps lossless PNG data alongside standard bitmap on clipboard",
            "Standart bitmap'in yanında kayıpsız PNG verisini de panoya ekler",
        )
    }
    pub fn lbl_copy_and_save(&self) -> &'static str {
        self.pick(
            "Auto-save file when copying",
            "Kopyalarken otomatik dosyaya kaydet",
        )
    }
    pub fn desc_copy_and_save(&self) -> &'static str {
        self.pick(
            "Pressing Enter/Copy during capture also writes a PNG to disk",
            "Yakalama sırasında Kopyala/Enter yapıldığında diske de PNG kaydeder",
        )
    }

    // Tray menu
    pub fn tray_capture(&self) -> &'static str {
        self.pick("Start / cancel capture", "Yakalama başlat / iptal")
    }
    pub fn tray_studio(&self) -> &'static str {
        self.pick("Open xscreen", "xscreen'i aç")
    }
    pub fn tray_editor(&self) -> &'static str {
        self.pick("Image editor", "Görsel düzenleyici")
    }
    pub fn tray_last(&self) -> &'static str {
        self.pick("Open last capture", "Son yakalamayı aç")
    }
    pub fn tray_folder(&self) -> &'static str {
        self.pick("Show capture folder", "Kayıt klasörünü göster")
    }
    pub fn tray_settings(&self) -> &'static str {
        self.pick("Settings", "Ayarlar")
    }
    pub fn tray_exit(&self) -> &'static str {
        self.pick("Exit", "Çıkış")
    }
}
