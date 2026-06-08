// Media Manager - All media related functions
// Handles upload, edit, delete, library, and ordering

window.createMediaManager = function(form, contentId) {
    const { ref, computed, nextTick, watch } = Vue;
    
    // ============ STATE ============
    const uploadingMedia = ref({});
    const editingMedia = ref(null);
    const editingMediaLang = ref(null);
    const editingMediaIndex = ref(null);
    const editMediaFile = ref(null);
    let mediaModal = null;

    
    // Media library
    const libraryMedia = ref([]);
    const libraryMeta = ref({});
    const loadingLibrary = ref(false);
    const selectedLibraryMedia = ref([]);
    const currentLibraryLang = ref(null);
    const librarySearchTimeout = ref(null);
    let libraryModal = null;
    
    const libraryFilters = ref({
        search: '',
        media_type: '',
        page: 1,
        limit: 24
    });

    
    const get_media_images_thumbnail = (path) => {

        if (!path || path.trim() === '') return '/static/no_image.png';


        let clean = String(path);
        clean = clean.replace(/^\/media\/uploads\//, '');
        clean = clean.replace(/^media\/uploads\//, '');
        clean = clean.replace(/^\/media\//, '');

        return `/media/thumb/150x150/center/${clean}`;
    };
    
    // ============ COMPUTED ============
    const groupedMedia = computed(() => {
        return (langCode) => {
            // Güvenlik kontrolü: form.value.data[langCode] yoksa boş döndür
            if (!form.value.data || !form.value.data[langCode]) {
                return {
                    cover: [],
                    icon: [],
                    video: [],
                    gallery: [],
                    document: []
                };
            }
            
            const media = form.value.data[langCode].media;
            
            // Eğer media object ise (yeni format), direkt döndür
            if (media && typeof media === 'object' && !Array.isArray(media)) {
                return {
                    cover: (media.cover || []).sort((a, b) => (a.order_id || 0) - (b.order_id || 0)),
                    icon: (media.icon || []).sort((a, b) => (a.order_id || 0) - (b.order_id || 0)),
                    video: (media.video || []).sort((a, b) => (a.order_id || 0) - (b.order_id || 0)),
                    gallery: (media.gallery || []).sort((a, b) => (a.order_id || 0) - (b.order_id || 0)),
                    document: (media.document || []).sort((a, b) => (a.order_id || 0) - (b.order_id || 0))
                };
            }
            
            // Eğer media array ise (eski format), migrate et
            if (Array.isArray(media)) {
                const groups = {
                    cover: media.filter(m => m.media_type === 'cover').sort((a, b) => (a.order_id || 0) - (b.order_id || 0)),
                    icon: media.filter(m => m.media_type === 'icon').sort((a, b) => (a.order_id || 0) - (b.order_id || 0)),
                    video: media.filter(m => m.media_type === 'video').sort((a, b) => (a.order_id || 0) - (b.order_id || 0)),
                    gallery: media.filter(m => m.media_type === 'gallery').sort((a, b) => (a.order_id || 0) - (b.order_id || 0)),
                    document: media.filter(m => m.media_type === 'document').sort((a, b) => (a.order_id || 0) - (b.order_id || 0))
                };
                // Otomatik olarak yeni formata çevir
                form.value.data[langCode].media = groups;
                return groups;
            }
            
            // Hiç media yoksa boş object döndür
            return {
                cover: [],
                icon: [],
                video: [],
                gallery: [],
                document: []
            };
        };
    });
    
    // ============ METHODS ============
    
    // Upload media files
    const handleMediaUpload = async (event, langCode) => {
        const files = Array.from(event.target.files);
        if (files.length === 0) return;

        uploadingMedia.value[langCode] = true;

        try {
            const formData = new FormData();
            files.forEach(file => formData.append('files', file));

            const response = await fetch(`/admin/api/media?content_type=pages&content_id=${contentId}`, {
                method: 'POST',
                body: formData
            });

            if (response.ok) {
                const data = await response.json();

                if (data.uploaded && data.uploaded.length > 0) {
                    // Media yapısını initialize et (object formatında)
                    if (!form.value.data[langCode].media || Array.isArray(form.value.data[langCode].media)) {
                        form.value.data[langCode].media = {
                            cover: [],
                            icon: [],
                            video: [],
                            gallery: [],
                            document: []
                        };
                    }

                    data.uploaded.forEach((media, index) => {
                        // Kategori belirleme: mime type'a göre akıllı kategorilendirme
                        let mediaCategory = mapMediaType(media.media_type, media.mime_type);
                        
                        // Eğer bu dilde hiç cover yoksa ve bu bir resimse, ilk resmi cover yap
                        if (mediaCategory === 'gallery' && 
                            index === 0 && 
                            form.value.data[langCode].media.cover.length === 0) {
                            mediaCategory = 'cover';
                        }

                        // İlgili kategorideki mevcut medyaların max order_id'sini bul
                        const categoryMedia = form.value.data[langCode].media[mediaCategory] || [];
                        const maxOrder = categoryMedia.length > 0
                            ? Math.max(...categoryMedia.map(m => m.order_id || 0))
                            : 0;

                        // Kategoriye göre ilgili array'e ekle
                        if (!form.value.data[langCode].media[mediaCategory]) {
                            form.value.data[langCode].media[mediaCategory] = [];
                        }

                        form.value.data[langCode].media[mediaCategory].push({
                            id: media.id,
                            file_name: media.file_name,
                            url: media.url,
                            mime_type: media.mime_type,
                            title: '',
                            description: '',
                            content: '',
                            order_id: maxOrder + 1
                        });
                    });

                    // Otomatik kaydetme için event fırlat (sessiz)
                    document.dispatchEvent(new CustomEvent('content-media-updated', {
                        detail: { lang: langCode }
                    }));
                }

                if (data.errors && data.errors.length > 0) {
                    Swal.fire({
                        icon: 'warning',
                        title: 'Uyarı',
                        text: `Bazı dosyalar yüklenemedi: ${data.errors.join(', ')}`,
                        toast: true,
                        position: 'top-end',
                        showConfirmButton: false,
                        timer: 5000
                    });
                }
            } else {
                const errorMessage = await getErrorMessage(response, 'Dosyalar yüklenirken hata oluştu');
                Swal.fire({
                    icon: 'error',
                    title: 'Hata!',
                    text: errorMessage,
                    toast: true,
                    position: 'top-end',
                    showConfirmButton: false,
                    timer: 7000
                });
            }
        } catch (error) {
            Swal.fire({
                icon: 'error',
                title: 'Hata!',
                text: 'Dosyalar yüklenirken hata oluştu: ' + error.message,
                toast: true,
                position: 'top-end',
                showConfirmButton: false,
                timer: 7000
            });
        } finally {
            uploadingMedia.value[langCode] = false;
            event.target.value = '';
        }
    };
    
    // Remove media
    const removeMedia = async (langCode, category, index) => {
        const result = await Swal.fire({
            title: 'Emin misiniz?',
            text: 'Bu medya silinecek!',
            icon: 'warning',
            showCancelButton: true,
            confirmButtonText: 'Evet, sil!',
            cancelButtonText: 'İptal'
        });

        if (result.isConfirmed) {
            const mediaToDelete = form.value.data[langCode].media[category][index];

            try {
                const response = await fetch(`/admin/api/media/${mediaToDelete.id}`, {
                    method: 'DELETE'
                });

                if (response.ok) {
                    form.value.data[langCode].media[category].splice(index, 1);

                    // Otomatik kaydetme için event fırlat (sessiz)
                    document.dispatchEvent(new CustomEvent('content-media-updated', { detail: { lang: langCode } }));
                } else if (response.status === 404) {
                    form.value.data[langCode].media[category].splice(index, 1);

                    // Otomatik kaydetme için event fırlat (sessiz)
                    document.dispatchEvent(new CustomEvent('content-media-updated', { detail: { lang: langCode } }));
                } else {
                    const errorMessage = await getErrorMessage(response, 'Medya silinirken hata oluştu');
                    Swal.fire({
                        icon: 'error',
                        title: 'Hata!',
                        text: errorMessage || 'Medya silinirken hata oluştu',
                        toast: true,
                        position: 'top-end',
                        showConfirmButton: false,
                        timer: 7000
                    });
                }
            } catch (error) {
                Swal.fire({
                    icon: 'error',
                    title: 'Hata!',
                    text: 'Medya silinirken hata oluştu: ' + error.message,
                    toast: true,
                    position: 'top-end',
                    showConfirmButton: false,
                    timer: 7000
                });
            }
        }
    };
    
    // Open media edit modal
    const openMediaModal = (langCode, category, index) => {
        editingMediaLang.value = langCode;
        editingMediaIndex.value = { category, index };
        editingMedia.value = { 
            ...form.value.data[langCode].media[category][index],
            media_type: category // Mevcut kategoriyi de sakla
        };

        // reset search state when opening
        searchTerm.value = '';
        searchResults.value = [];
        searchLoading.value = false;
        selectedSearchId.value = null;

        editMediaFile.value = null;

        if (!mediaModal) {
            mediaModal = new bootstrap.Modal(document.getElementById('mediaEditModal'));
        }
        mediaModal.show();
        console.log('Editing media widget content:', editingMedia.value);

       
    };
    
    // Handle media file select for editing
    const handleEditMediaFileSelect = (event) => {
        const file = event.target.files[0];
        if (file) {
            editMediaFile.value = file;
        }
    };

    
    // Save media edit
    const saveMediaEdit = async () => {
        if (editingMedia.value && editingMediaLang.value !== null && editingMediaIndex.value !== null) {
            const { category: oldCategory, index } = editingMediaIndex.value;
            const newCategory = editingMedia.value.media_type;
            
            if (editMediaFile.value) {
                // File replacement mode
                const currentMedia = { ...editingMedia.value };

                if (!currentMedia.id) {
                    Swal.fire({
                        icon: 'error',
                        title: 'Hata!',
                        text: 'Medya ID bulunamadı. Dosya değiştirilemez.',
                        toast: true,
                        position: 'top-end',
                        showConfirmButton: false,
                        timer: 7000
                    });
                    return;
                }

                try {
                    const formData = new FormData();
                    formData.append('file', editMediaFile.value);
                    formData.append('title', editingMedia.value.title || '');
                    formData.append('description', editingMedia.value.description || '');

                    const response = await fetch(`/admin/api/media/${currentMedia.id}`, {
                        method: 'PUT',
                        body: formData
                    });

                    if (response.ok) {
                        const updatedMedia = await response.json();

                        const updatedMediaData = {
                            id: updatedMedia.id,
                            file_name: updatedMedia.file_name,
                            url: updatedMedia.url,
                            mime_type: updatedMedia.mime_type,
                            title: editingMedia.value.title || '',
                            description: editingMedia.value.description || '',
                            content: editingMedia.value.content || '',
                            link: editingMedia.value.link || '',
                            order_id: form.value.data[editingMediaLang.value].media[oldCategory][index].order_id
                        };

                        // Kategori değiştiyse, eski kategoriden sil ve yeni kategoriye ekle
                        if (oldCategory !== newCategory) {
                            form.value.data[editingMediaLang.value].media[oldCategory].splice(index, 1);
                            if (!form.value.data[editingMediaLang.value].media[newCategory]) {
                                form.value.data[editingMediaLang.value].media[newCategory] = [];
                            }
                            form.value.data[editingMediaLang.value].media[newCategory].push(updatedMediaData);
                        } else {
                            // Aynı kategoride güncelle
                            form.value.data[editingMediaLang.value].media[oldCategory][index] = updatedMediaData;
                        }

                        // Clear file input
                        editMediaFile.value = null;
                        
                        mediaModal.hide();

                        // Otomatik olarak içeriği kaydetmek için event fırlat
                        document.dispatchEvent(new CustomEvent('content-media-updated', {
                            detail: { lang: editingMediaLang.value }
                        }));
                    } else {
                        const errorMessage = await getErrorMessage(response, 'Medya güncellenirken hata oluştu');
                        Swal.fire({
                            icon: 'error',
                            title: 'Hata!',
                            text: errorMessage,
                            toast: true,
                            position: 'top-end',
                            showConfirmButton: false,
                            timer: 7000
                        });
                    }
                } catch (error) {
                    Swal.fire({
                        icon: 'error',
                        title: 'Hata!',
                        text: 'Medya güncellenirken hata oluştu: ' + error.message,
                        toast: true,
                        position: 'top-end',
                        showConfirmButton: false,
                        timer: 7000
                    });
                }
            } else {
                // Metadata only update
                const updatedMediaData = {
                    ...form.value.data[editingMediaLang.value].media[oldCategory][index],
                    title: editingMedia.value.title || '',
                    description: editingMedia.value.description || '',
                    content: editingMedia.value.content || '',
                    link: editingMedia.value.link || '',
                };

                // Kategori değiştiyse, eski kategoriden sil ve yeni kategoriye ekle
                if (oldCategory !== newCategory) {
                    form.value.data[editingMediaLang.value].media[oldCategory].splice(index, 1);
                    if (!form.value.data[editingMediaLang.value].media[newCategory]) {
                        form.value.data[editingMediaLang.value].media[newCategory] = [];
                    }
                    form.value.data[editingMediaLang.value].media[newCategory].push(updatedMediaData);
                } else {
                    // Aynı kategoride güncelle
                    form.value.data[editingMediaLang.value].media[oldCategory][index] = updatedMediaData;
                }

                mediaModal.hide();

                // Otomatik olarak içeriği kaydetmek için event fırlat
                document.dispatchEvent(new CustomEvent('content-media-updated', {
                    detail: { lang: editingMediaLang.value }
                }));
            }
        }
    };
    
    // Remove media from modal
    const removeMediaFromModal = async () => {
        const result = await Swal.fire({
            title: 'Emin misiniz?',
            text: 'Bu medya silinecek!',
            icon: 'warning',
            showCancelButton: true,
            confirmButtonText: 'Evet, sil!',
            cancelButtonText: 'İptal'
        });

        if (result.isConfirmed) {
            const { category, index } = editingMediaIndex.value;
            const mediaToDelete = form.value.data[editingMediaLang.value].media[category][index];

            try {
                const response = await fetch(`/admin/api/media/${mediaToDelete.id}`, {
                    method: 'DELETE'
                });

                if (response.ok) {
                    form.value.data[editingMediaLang.value].media[category].splice(index, 1);
                    mediaModal.hide();

                    // Otomatik kaydetme için event fırlat (sessiz)
                    document.dispatchEvent(new CustomEvent('content-media-updated', { detail: { lang: editingMediaLang.value } }));
                } else if (response.status === 404) {
                    form.value.data[editingMediaLang.value].media[category].splice(index, 1);
                    mediaModal.hide();

                    // Otomatik kaydetme için event fırlat (sessiz)
                    document.dispatchEvent(new CustomEvent('content-media-updated', { detail: { lang: editingMediaLang.value } }));
                } else {
                    const errorMessage = await getErrorMessage(response, 'Medya silinirken hata oluştu');
                    Swal.fire({
                        icon: 'error',
                        title: 'Hata!',
                        text: errorMessage || 'Medya silinirken hata oluştu',
                        toast: true,
                        position: 'top-end',
                        showConfirmButton: false,
                        timer: 7000
                    });
                }
            } catch (error) {
                Swal.fire({
                    icon: 'error',
                    title: 'Hata!',
                    text: 'Medya silinirken hata oluştu: ' + error.message,
                    toast: true,
                    position: 'top-end',
                    showConfirmButton: false,
                    timer: 7000
                });
            }
        }
    };
    
    // ============ MEDIA LIBRARY ============
    
    const openMediaLibrary = (langCode) => {
        currentLibraryLang.value = langCode;
        selectedLibraryMedia.value = [];
        loadLibraryMedia();

        if (!libraryModal) {
            libraryModal = new bootstrap.Modal(document.getElementById('mediaLibraryModal'));
        }
        libraryModal.show();
    };
    
    const loadLibraryMedia = async () => {
        loadingLibrary.value = true;
        try {
            const params = new URLSearchParams({
                page: libraryFilters.value.page,
                limit: libraryFilters.value.limit,
                ...(libraryFilters.value.search && { search: libraryFilters.value.search }),
                ...(libraryFilters.value.media_type && { media_type: libraryFilters.value.media_type })
            });

            const response = await fetch(`/admin/api/media?${params}`);
            const data = await response.json();

            if (response.ok) {
                libraryMedia.value = data.data || [];
                libraryMeta.value = data.meta || {};
            }
        } catch (error) {
            console.error('Media library yüklenirken hata:', error);
        } finally {
            loadingLibrary.value = false;
        }
    };
    
    const debounceLibrarySearch = () => {
        clearTimeout(librarySearchTimeout.value);
        librarySearchTimeout.value = setTimeout(() => {
            libraryFilters.value.page = 1;
            loadLibraryMedia();
        }, 500);
    };
    
    const changeLibraryPage = (page) => {
        if (page >= 1 && page <= libraryMeta.value.total_pages) {
            libraryFilters.value.page = page;
            loadLibraryMedia();
        }
    };
    
    const toggleLibraryMedia = (media) => {
        const index = selectedLibraryMedia.value.indexOf(media.id);
        if (index > -1) {
            selectedLibraryMedia.value.splice(index, 1);
        } else {
            selectedLibraryMedia.value.push(media.id);
        }
    };
    
    const addSelectedMediaToPage = async () => {
        if (selectedLibraryMedia.value.length === 0 || !currentLibraryLang.value) return;

        // contentId must be a valid number for cloning
        if (!contentId || contentId === null) {
            Swal.fire({
                icon: 'warning',
                title: 'Uyarı!',
                text: 'Medya kütüphanesinden eklemek için önce içeriği kaydetmelisiniz.',
                toast: true,
                position: 'top-end',
                showConfirmButton: false,
                timer: 5000
            });
            return;
        }

        try {
            const response = await fetch('/admin/api/media/clone', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    media_ids: selectedLibraryMedia.value,
                    content_type: 'pages',
                    content_id: parseInt(contentId)
                })
            });

            if (response.ok) {
                const data = await response.json();

                if (data.cloned && data.cloned.length > 0) {
                    // Media yapısını initialize et (object formatında)
                    if (!form.value.data[currentLibraryLang.value].media || Array.isArray(form.value.data[currentLibraryLang.value].media)) {
                        form.value.data[currentLibraryLang.value].media = {
                            cover: [],
                            icon: [],
                            video: [],
                            gallery: [],
                            document: []
                        };
                    }

                    data.cloned.forEach(media => {
                        // Mime type'a göre kategori belirle
                        const mediaCategory = mapMediaType(media.media_type, media.mime_type);

                        // İlgili kategorideki mevcut medyaların max order_id'sini bul
                        const categoryMedia = form.value.data[currentLibraryLang.value].media[mediaCategory] || [];
                        const maxOrder = categoryMedia.length > 0
                            ? Math.max(...categoryMedia.map(m => m.order_id || 0))
                            : 0;

                        // Kategoriye göre ilgili array'e ekle
                        if (!form.value.data[currentLibraryLang.value].media[mediaCategory]) {
                            form.value.data[currentLibraryLang.value].media[mediaCategory] = [];
                        }

                        form.value.data[currentLibraryLang.value].media[mediaCategory].push({
                            id: media.id,
                            file_name: media.file_name,
                            url: media.url,
                            mime_type: media.mime_type,
                            title: media.title || '',
                            description: media.description || '',
                            link: media.link || '',
                            content: media.content || '',
                            order_id: maxOrder + 1
                        });
                    });

                    libraryModal.hide();

                    // Otomatik kaydetme için event fırlat (sessiz)
                    document.dispatchEvent(new CustomEvent('content-media-updated', {
                        detail: { lang: currentLibraryLang.value }
                    }));
                }
            } else {
                let errorMessage = 'Medya eklenirken hata oluştu';
                try {
                    // First try to read as text
                    const text = await response.text();
                    // Then try to parse as JSON
                    try {
                        const error = JSON.parse(text);
                        errorMessage = error.error || text || errorMessage;
                    } catch (e) {
                        // If not JSON, use text directly
                        errorMessage = text || errorMessage;
                    }
                } catch (e) {
                    // If reading fails, use default message
                    errorMessage = 'Medya eklenirken hata oluştu';
                }
                
                Swal.fire({
                    icon: 'error',
                    title: 'Hata!',
                    text: errorMessage,
                    toast: true,
                    position: 'top-end',
                    showConfirmButton: false,
                    timer: 7000
                });
            }
        } catch (error) {
            Swal.fire({
                icon: 'error',
                title: 'Hata!',
                text: 'Medya eklenirken hata oluştu: ' + error.message,
                toast: true,
                position: 'top-end',
                showConfirmButton: false,
                timer: 7000
            });
        }
    };
    
    // ============ UTILITY FUNCTIONS ============
    
    // Safe error message extraction from response
    const getErrorMessage = async (response, defaultMessage = 'Bir hata oluştu') => {
        try {
            const text = await response.text();
            try {
                const json = JSON.parse(text);
                return json.error || text || defaultMessage;
            } catch (e) {
                return text || defaultMessage;
            }
        } catch (e) {
            return defaultMessage;
        }
    };
    
    const getMediaIndex = (langCode, category, mediaId) => {
        const categoryMedia = form.value.data[langCode].media[category] || [];
        return categoryMedia.findIndex(m => m.id === mediaId);
    };
    
    const mapMediaType = (backendType, mimeType) => {
        // Eğer backendType zaten bir kategori ise (cover, icon, gallery, video, document), olduğu gibi döndür
        if (['cover', 'icon', 'gallery', 'video', 'document'].includes(backendType)) {
            return backendType;
        }
        
        // Mime type'a göre kategori belirle
        if (mimeType) {
            // Video dosyaları
            if (mimeType.startsWith('video/')) {
                return 'video';
            }
            // Döküman dosyaları (PDF, Word, Excel, vb.)
            if (mimeType === 'application/pdf' || 
                mimeType.includes('word') || 
                mimeType.includes('document') ||
                mimeType.includes('sheet') || 
                mimeType.includes('excel') ||
                mimeType.includes('presentation') || 
                mimeType.includes('powerpoint') ||
                mimeType.startsWith('text/')) {
                return 'document';
            }
            // Resim ve ses dosyaları galeri olarak
            if (mimeType.startsWith('image/') || mimeType.startsWith('audio/')) {
                return 'gallery';
            }
        }
        
        // Backend type'a göre fallback
        if (backendType === 'document') return 'document';
        if (backendType === 'video') return 'video';
        
        // Default olarak gallery
        return 'gallery';
    };
    
    const getMediaIcon = (mimeType) => {
        if (!mimeType) return 'bi-file-earmark';
        if (mimeType.startsWith('video/')) return 'bi-file-earmark-play';
        if (mimeType.startsWith('audio/')) return 'bi-file-earmark-music';
        if (mimeType === 'application/pdf') return 'bi-file-earmark-pdf';
        if (mimeType.includes('word') || mimeType.includes('document')) return 'bi-file-earmark-word';
        if (mimeType.includes('sheet') || mimeType.includes('excel')) return 'bi-file-earmark-excel';
        if (mimeType.includes('presentation') || mimeType.includes('powerpoint')) return 'bi-file-earmark-ppt';
        if (mimeType.startsWith('text/')) return 'bi-file-earmark-text';
        return 'bi-file-earmark';
    };
    
    const getMediaIconColor = (mimeType) => {
        if (!mimeType) return 'text-muted';
        if (mimeType.startsWith('video/')) return 'text-primary';
        if (mimeType.startsWith('audio/')) return 'text-success';
        if (mimeType === 'application/pdf') return 'text-danger';
        if (mimeType.includes('word') || mimeType.includes('document')) return 'text-info';
        if (mimeType.includes('sheet') || mimeType.includes('excel')) return 'text-success';
        if (mimeType.includes('presentation') || mimeType.includes('powerpoint')) return 'text-warning';
        return 'text-muted';
    };
    
    const updateMediaOrder = (langCode, category) => {
        const el = document.getElementById(`sortable-${category}-${langCode}`);
        if (!el) return;

        const items = el.querySelectorAll('.sortable-item');
        const newOrder = Array.from(items).map(item => parseInt(item.dataset.id));

        const categoryMedia = form.value.data[langCode].media[category] || [];

        const reorderedMedia = newOrder.map((id, index) => {
            const media = categoryMedia.find(m => m.id === id);
            if (media) {
                media.order_id = index + 1;
            }
            return media;
        }).filter(Boolean);

        form.value.data[langCode].media[category] = reorderedMedia;
    };
    
    // Initialize Sortable for media items
    const initSortable = () => {
        nextTick(() => {
            if (!form.value.data) return;
            
            Object.keys(form.value.data).forEach(langCode => {
                if (!form.value.data[langCode]) return;
                
                ['cover', 'icon', 'video', 'gallery', 'document'].forEach(type => {
                    const el = document.getElementById(`sortable-${type}-${langCode}`);
                    if (el && !el.sortableInstance) {
                        el.sortableInstance = new Sortable(el, {
                            animation: 150,
                            handle: '.sortable-handle',
                            onEnd: () => updateMediaOrder(langCode, type)
                        });
                    }
                });
            });
        });
    };
    
    // Watch for media changes to reinitialize sortable
    watch(() => form.value.data, () => {
        initSortable();
    }, { deep: true });

    // Admin search state
    const searchTerm = ref('');
    const searchResults = ref([]);
    const searchLoading = ref(false);
    const searchTimeout = ref(null);
    const selectedSearchId = ref(null);

    const performSearch = async () => {
        const q = (searchTerm.value || '').trim();
        if (q.length === 0) {
            searchResults.value = [];
            selectedSearchId.value = null;
            return;
        }

        searchLoading.value = true;
        searchResults.value = [];
        selectedSearchId.value = null;

        try {
            const module = 'all';
            // Request multilingual search results (server will return titles/descriptions and absolute_urls per language)
            const params = new URLSearchParams({ q, module, lang: editingMediaLang.value || 'all' });
            const res = await fetch(`/admin/api/build-content-absolute-url/search?${params.toString()}`);
            if (res.ok) {
                const j = await res.json();
                searchResults.value = j.results || [];
            }
        } catch (e) {
            console.error('Search error', e);
        } finally {
            searchLoading.value = false;
        }
    };

    const debouncedSearch = () => {
        clearTimeout(searchTimeout.value);
        searchTimeout.value = setTimeout(() => performSearch(), 300);
    };

    const selectSearchResult = (r) => {
        if (!r) return;
        selectedSearchId.value = r.id;

        if (editingMedia.value) {
            // Prefer per-language absolute URL for the current editing language, fallback to first available or r.url
            const lang = editingMediaLang.value;
            let foundUrl = '';
            if (r.absolute_urls) {
                if (r.absolute_urls[lang]) {
                    foundUrl = r.absolute_urls[lang];
                } else {
                    // fallback to first non-empty absolute_urls value
                    const first = Object.values(r.absolute_urls).find(v => typeof v === 'string' && v.length > 0);
                    if (first) foundUrl = first;
                }
            }

            if (!foundUrl) {
                foundUrl = r.url || '';
            }

            editingMedia.value.link = foundUrl || '';
        }

        Swal.fire({ icon: 'success', title: 'Seçildi', text: `${(r.titles && r.titles[editingMediaLang.value]) ? r.titles[editingMediaLang.value] : r.title} seçildi ve link kutusuna yerleştirildi`, toast: true, position: 'top-end', showConfirmButton: false, timer: 1800 });
    };

    const copyToClipboard = async (text) => {
        try {
            if (navigator.clipboard && text) {
                await navigator.clipboard.writeText(text);
                Swal.fire({ icon: 'success', title: 'Kopyalandı', toast: true, position: 'top-end', showConfirmButton: false, timer: 1500 });
            }
        } catch (e) {
            console.error('Clipboard error', e);
        }
    };

    // Return all methods and state
    return {
        // State
        uploadingMedia,
        editingMedia,
        editingMediaLang,
        editingMediaIndex,
        editMediaFile,
        libraryMedia,
        libraryMeta,
        loadingLibrary,
        
        // search exports
        searchTerm,
        searchResults,
        searchLoading,
        debouncedSearch,
        performSearch,
        selectSearchResult,
        selectedSearchId,
        selectedLibraryMedia,
        currentLibraryLang,
        libraryFilters,
        
        // Computed
        groupedMedia,
        
        // Methods
        handleMediaUpload,
        removeMedia,
        openMediaModal,
        handleEditMediaFileSelect,
        saveMediaEdit,
        removeMediaFromModal,
        openMediaLibrary,
        loadLibraryMedia,
        debounceLibrarySearch,
        changeLibraryPage,
        toggleLibraryMedia,
        addSelectedMediaToPage,
        getMediaIndex,
        mapMediaType,
        getMediaIcon,
        getMediaIconColor,
        updateMediaOrder,
        initSortable,

        copyToClipboard,
        get_media_images_thumbnail
    };
};
