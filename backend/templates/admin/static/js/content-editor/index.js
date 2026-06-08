// Content Editor - Main Entry Point
// Bu dosya tüm modülleri birleştirir ve Vue app'i oluşturur

// NOT: Bu dosya şu an kullanılmıyor
// Tüm kod hala templates/admin/contents/partials/script.html içinde
// TODO: script.html'deki kodu buraya taşı

function createContentEditor(contentId = null) {
    const { createApp, ref, computed, onMounted, watch, nextTick } = Vue;

    return createApp({
        delimiters: ["[[", "]]"],
        components: {
            CategoryItem: CategoryItemComponent
        },
        setup() {
            // State, computed, methods buraya gelecek
            // Şimdilik script.html'den include ediliyor
            
            return {
                // Tüm metodlar ve state return edilecek
            };
        }
    });
}

// Global olarak kullanılabilir yap
window.createContentEditor = createContentEditor;
