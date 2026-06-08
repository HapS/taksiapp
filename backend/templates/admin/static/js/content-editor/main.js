// içerik editörü ana uygulaması
// kullanım: window.initContentEditor(contentId)

window.initContentEditor = function (contentId = null) {
    const {
        createApp,
        ref,
        computed,
        onMounted,
        onBeforeUnmount,
        watch,
        nextTick,
    } = Vue;

    const app = createApp({
        delimiters: ["[[", "]]"],
        components: {
            // bileşenler ayrı dosyalardan yüklenir
            CategoryItem: CategoryItemComponent,
            CategoryOption: CategoryOptionComponent,
            FormBuilder: FormBuilderComponent,
        },
        setup() {
            // reaktif durum
            const loading = ref(false);
            const saving = ref(false);
            // contentId parametre olarak geçilir
            const isEditing = computed(() => contentId !== null);

            // desteklenen diller (api'den dinamik)
            const supportedLanguages = ref({});
            const defaultLanguage = ref("tr");
            const loadingLanguages = ref(false);
            const availableTemplates = ref([]);

            // default templates per content type
            const defaultTemplates = {
                page: "page_detail.html",
                product: "product_detail.html",
                blog: "blog_detail.html",
                form: "form_standart.html",
                news: "news_detail.html",
            };

            // hesaplanmış: varsayılan dil önce olmak üzere sıralanmış diller
            const sortedLanguages = computed(() => {
                const langs = supportedLanguages.value;
                if (!langs || Object.keys(langs).length === 0) return {};

                const sorted = {};
                const defaultLang = defaultLanguage.value;

                // önce varsayılan dili ekle
                if (langs[defaultLang]) {
                    sorted[defaultLang] = langs[defaultLang];
                }

                // sonra diğer dilleri ekle
                Object.keys(langs).forEach((langCode) => {
                    if (langCode !== defaultLang) {
                        sorted[langCode] = langs[langCode];
                    }
                });

                return sorted;
            });

            // URL'den parametreleri al
            const urlParams = new URLSearchParams(window.location.search);
            const parentId = urlParams.get("parent_id");
            const contentType = urlParams.get("content_type")?.toLowerCase();

            const form = ref({
                content_type: contentType || "page",
                publish: true,
                gcx: false, // Global Context
                data: {}, // loadLanguages'de başlatılacak
                template:
                    defaultTemplates[contentType] || defaultTemplates["page"],
                icon: "",
                color1: "#ffffff",
                color2: "#000000",
                color3: "#0d6efd",

                parent_id: parentId ? parseInt(parentId) : null,
                term_master_id: null, // ana kategori id
                term_ids: [], // ikincil kategori id'leri
                tag_ids: [],
                settings: {
                    kapak_resmi_goster: true,
                    kapak_yazi_goster: true,
                },
                sub_contents: [],
                form_settings: {
                    allowAnonymous: true,
                    sendEmail: false,
                    title: true,
                    description: true,
                    fields: [],
                },
                product: {
                    currency: "TRY",
                    price: 0,
                    b2b_price: 0,
                    sku: "",
                    stock: 0,
                    on_sale: false,
                    attributes: {}, // yeni: taksonomi özellikleri
                    // Pazaryeri entegrasyon alanları
                    barcode: "",
                    vat_rate: 20,
                    weight: null,
                    dimensional_weight: null,
                    dimensions: {
                        width: null,
                        height: null,
                        depth: null,
                    },
                    delivery_duration: 3,
                    discount_percentage: 0,
                },
            });

            const parentPage = ref(null);
            const pageBreadcrumbs = ref([]);
            const availableParentPagesRaw = ref([]);
            const loadingParents = ref(false);
            const parentSearch = ref("");
            const parentSearchTimeout = ref(null);
            let parentModal = null;

            // ürün varyant medya durumu
            const editingVariant = ref(null);
            const uploadingVariantMedia = ref(false);
            let variantMediaModal = null;

            // hesaplanmış: hiyerarşik olarak sıralanmış üst sayfalar
            const availableParentPages = computed(() => {
                let pages = availableParentPagesRaw.value;

                // hiyerarşik sıralama: önce kök öğeler, sonra üst öğeye göre gruplandırılmış alt öğeler
                const sortedPages = [];

                // önce kök öğeleri ekle (üst öğesi olmayan)
                const rootPages = pages
                    .filter((p) => !p.parent_id)
                    .sort((a, b) => {
                        const aTitle = getPageTitle(a);
                        const bTitle = getPageTitle(b);
                        return aTitle.localeCompare(bTitle, "tr");
                    });

                // alt öğeleri özyinelemeli olarak ekle
                const addChildrenRecursive = (parentId, level = 0) => {
                    const children = pages
                        .filter((p) => p.parent_id === parentId)
                        .sort((a, b) => {
                            const aTitle = getPageTitle(a);
                            const bTitle = getPageTitle(b);
                            return aTitle.localeCompare(bTitle, "tr");
                        });

                    for (const child of children) {
                        child._level = level; // girinti için seviye ekle
                        sortedPages.push(child);
                        addChildrenRecursive(child.id, level + 1);
                    }
                };

                for (const root of rootPages) {
                    root._level = 0;
                    sortedPages.push(root);
                    addChildrenRecursive(root.id, 1);
                }

                return sortedPages;
            });

            // kategori durumu
            const availableTerms = ref([]);
            const loadingTerms = ref(false);

            // etiket durumu
            const availableTags = ref([]);
            const selectedTags = ref([]);
            const loadingTags = ref(false);
            const tagSearch = ref("");
            const tagSuggestions = ref([]);
            const selectedTagIndex = ref(0);
            const tagSearchTimeout = ref(null);

            const generateSlug = (langCode) => {
                const title = form.value.data[langCode].title;
                if (!title) return;

                // Türkçe karakterleri dönüştür
                const trMap = {
                    ç: "c",
                    Ç: "C",
                    ğ: "g",
                    Ğ: "G",
                    ı: "i",
                    İ: "I",
                    ö: "o",
                    Ö: "O",
                    ş: "s",
                    Ş: "S",
                    ü: "u",
                    Ü: "U",
                };

                let slug = title.toLowerCase();
                Object.keys(trMap).forEach((key) => {
                    slug = slug.replace(new RegExp(key, "g"), trMap[key]);
                });

                slug = slug
                    .replace(/[^a-z0-9\s-]/g, "")
                    .replace(/\s+/g, "-")
                    .replace(/-+/g, "-")
                    .replace(/^-+|-+$/g, "");

                form.value.data[langCode].slug = slug;
            };

            const loadContent = async () => {
                if (!isEditing.value) return;

                loading.value = true;
                try {
                    const response = await fetch(
                        `/admin/api/contents/${contentId}`,
                    );
                    const pageData = await response.json();

                    if (response.ok) {
                        form.value.publish = pageData.publish || false;
                        form.value.gcx = pageData.gcx || false;
                        form.value.parent_id = pageData.parent_id || null;
                        form.value.content_type =
                            pageData.content_type || "page";

                        // Breadcrumb'ı API'den al
                        if (
                            pageData.breadcrumbs &&
                            Array.isArray(pageData.breadcrumbs)
                        ) {
                            pageBreadcrumbs.value = pageData.breadcrumbs;
                        } else {
                            pageBreadcrumbs.value = [];
                        }

                        // Term ve tag ID'lerini geçici olarak sakla
                        // loadTerms ve loadTags çağrıldıktan sonra ayırılacak
                        const allTermIds = pageData.term_ids || [];
                        form.value._allTermIds = allTermIds; // Geçici alan

                        // Parent ID varsa parent bilgisini yükle
                        if (pageData.parent_id) {
                            await loadParentPage();
                        }

                        // Raw JSON data'yı parse et
                        if (pageData.data) {
                            // data.langs varsa (eski format)
                            if (pageData.data.langs) {
                                Object.keys(supportedLanguages.value).forEach(
                                    (langCode) => {
                                        if (pageData.data.langs[langCode]) {
                                            const langData =
                                                pageData.data.langs[langCode];
                                            form.value.data[langCode] = {
                                                title: langData.title || "",
                                                slug: langData.slug || "",
                                                description:
                                                    langData.description || "",
                                                body: langData.body || "",
                                                meta_title:
                                                    langData.meta_title || "",
                                                meta_description:
                                                    langData.meta_description ||
                                                    "",
                                                media: langData.media || {
                                                    cover: [],
                                                    icon: [],
                                                    video: [],
                                                    gallery: [],
                                                    document: [],
                                                },
                                            };
                                        } else {
                                            // Bu dil için veri yoksa boş initialize et
                                            form.value.data[langCode] = {
                                                title: "",
                                                slug: "",
                                                description: "",
                                                body: "",
                                                meta_title: "",
                                                meta_description: "",
                                                media: {
                                                    cover: [],
                                                    icon: [],
                                                    video: [],
                                                    gallery: [],
                                                    document: [],
                                                },
                                            };
                                        }
                                    },
                                );

                                // Dil bağımsız alanlar
                                form.value.template =
                                    pageData.data.template &&
                                    pageData.data.template !== "default"
                                        ? pageData.data.template
                                        : defaultTemplates[
                                              form.value.content_type
                                          ] || defaultTemplates["page"];
form.value.icon =
                                    pageData.data.icon || "";
                                form.value.color1 =
                                    pageData.data.color1 || "#ffffff";
                                form.value.color2 =
                                    pageData.data.color2 || "#000000";
                                form.value.color3 =
                                    pageData.data.color3 || "#0d6efd";
                                form.value.settings =
                                    pageData.data.settings ||
                                    form.value.settings;
                                form.value.sub_contents =
                                    pageData.data.sub_contents || [];
                                form.value.term_master_id =
                                    pageData.data.term_master_id || null; // Ana kategori ID'si data içinde
                                form.value.form_settings =
                                    pageData.data.form_settings ||
                                    form.value.form_settings; // Form ayarları
                            } else {
                                // Yeni format - data direkt olarak lang kodlarını içeriyor
                                Object.keys(supportedLanguages.value).forEach(
                                    (langCode) => {
                                        if (pageData.data[langCode]) {
                                            const langData =
                                                pageData.data[langCode];
                                            form.value.data[langCode] = {
                                                title: langData.title || "",
                                                slug: langData.slug || "",
                                                description:
                                                    langData.description || "",
                                                body: langData.body || "",
                                                meta_title:
                                                    langData.meta_title || "",
                                                meta_description:
                                                    langData.meta_description ||
                                                    "",
                                                media: langData.media || {
                                                    cover: [],
                                                    icon: [],
                                                    video: [],
                                                    gallery: [],
                                                    document: [],
                                                },
                                            };
                                        } else {
                                            // Bu dil için veri yoksa boş initialize et
                                            form.value.data[langCode] = {
                                                title: "",
                                                slug: "",
                                                description: "",
                                                body: "",
                                                meta_title: "",
                                                meta_description: "",
                                                media: {
                                                    cover: [],
                                                    icon: [],
                                                    video: [],
                                                    gallery: [],
                                                    document: [],
                                                },
                                            };
                                        }
                                    },
                                );

                                // Dil bağımsız alanlar
                                form.value.template =
                                    pageData.data.template &&
                                    pageData.data.template !== "default"
                                        ? pageData.data.template
                                        : defaultTemplates[
                                              form.value.content_type
                                          ] || defaultTemplates["page"];
                                form.value.icon =
                                    pageData.data.icon || "";
                                form.value.color1 =
                                    pageData.data.color1 || "#ffffff";
                                form.value.color2 =
                                    pageData.data.color2 || "#000000";
                                form.value.color3 =
                                    pageData.data.color3 || "#0d6efd";
                                form.value.settings =
                                    pageData.data.settings ||
                                    form.value.settings;
                                form.value.sub_contents =
                                    pageData.data.sub_contents || [];
                                form.value.term_master_id =
                                    pageData.data.term_master_id || null; // Ana kategori ID'si
                                form.value.form_settings =
                                    pageData.data.form_settings ||
                                    form.value.form_settings; // Form ayarları
                            }

                            // Mevcut product verisini al ve attributes alanını ekle
                            const existingProduct = pageData.data.product || {};
                            form.value.product = {
                                currency: existingProduct.currency || "TRY",
                                price: existingProduct.price || 0,
                                b2b_price: existingProduct.b2b_price || 0,
                                sku: existingProduct.sku || "",
                                stock: existingProduct.stock || 0,
                                on_sale: existingProduct.on_sale || false,
                                attributes: existingProduct.attributes || {}, // YENİ: Taxonomy attributes
                                options: existingProduct.options || [],
                                variants: existingProduct.variants || [],
                                // Pazaryeri entegrasyon alanları
                                barcode: existingProduct.barcode || "",
                                vat_rate: existingProduct.vat_rate || 20,
                                weight: existingProduct.weight || null,
                                dimensional_weight:
                                    existingProduct.dimensional_weight || null,
                                dimensions: {
                                    width:
                                        existingProduct.dimensions?.width ||
                                        null,
                                    height:
                                        existingProduct.dimensions?.height ||
                                        null,
                                    depth:
                                        existingProduct.dimensions?.depth ||
                                        null,
                                },
                                delivery_duration:
                                    existingProduct.delivery_duration || 3,
                                discount_percentage: existingProduct.discount_percentage || 0,
                            };
                        }

                        // Migrate old array format to new object format and ensure order_id
                        Object.keys(form.value.data).forEach((langCode) => {
                            try {
                                // Güvenlik kontrolü: eğer bu dil için data yoksa atla
                                if (!form.value.data[langCode]) {
                                    return;
                                }

                                let media = form.value.data[langCode].media;

                                // Eğer media undefined veya null ise, boş object oluştur
                                if (!media) {
                                    form.value.data[langCode].media = {
                                        cover: [],
                                        icon: [],
                                        video: [],
                                        gallery: [],
                                        document: [],
                                    };
                                    return;
                                }

                                // Eğer media array ise (eski format), object'e çevir
                                if (Array.isArray(media)) {
                                    const newMedia = {
                                        cover: [],
                                        icon: [],
                                        video: [],
                                        gallery: [],
                                        document: [],
                                    };

                                    media.forEach((item, index) => {
                                        // order_id yoksa ekle
                                        if (!item.order_id) {
                                            item.order_id = index + 1;
                                        }

                                        // media_type'ı belirle
                                        let mediaType = item.media_type;
                                        if (
                                            ![
                                                "cover",
                                                "icon",
                                                "video",
                                                "gallery",
                                                "document",
                                            ].includes(mediaType)
                                        ) {
                                            // Backend'den gelen type'a göre map et
                                            if (
                                                item.mime_type &&
                                                item.mime_type.startsWith(
                                                    "video/",
                                                )
                                            ) {
                                                mediaType = "video";
                                            } else if (
                                                item.mime_type &&
                                                (item.mime_type ===
                                                    "application/pdf" ||
                                                    item.mime_type.includes(
                                                        "word",
                                                    ) ||
                                                    item.mime_type.includes(
                                                        "document",
                                                    ) ||
                                                    item.mime_type.includes(
                                                        "sheet",
                                                    ) ||
                                                    item.mime_type.includes(
                                                        "excel",
                                                    ))
                                            ) {
                                                mediaType = "document";
                                            } else {
                                                mediaType = "gallery";
                                            }
                                        }

                                        // media_type alanını kaldır (artık kategori olarak kullanılıyor)
                                        const {
                                            media_type,
                                            ...mediaWithoutType
                                        } = item;

                                        // İlgili kategoriye ekle
                                        newMedia[mediaType].push(
                                            mediaWithoutType,
                                        );
                                    });

                                    // Her kategoriyi order_id'ye göre sırala
                                    Object.keys(newMedia).forEach(
                                        (category) => {
                                            newMedia[category].sort(
                                                (a, b) =>
                                                    (a.order_id || 0) -
                                                    (b.order_id || 0),
                                            );
                                        },
                                    );

                                    form.value.data[langCode].media = newMedia;
                                } else if (media && typeof media === "object") {
                                    // Yeni format - sadece order_id kontrolü yap
                                    Object.keys(media).forEach((category) => {
                                        if (Array.isArray(media[category])) {
                                            media[category].forEach(
                                                (item, index) => {
                                                    if (!item.order_id) {
                                                        item.order_id =
                                                            index + 1;
                                                    }
                                                },
                                            );
                                            // Sırala
                                            media[category].sort(
                                                (a, b) =>
                                                    (a.order_id || 0) -
                                                    (b.order_id || 0),
                                            );
                                        }
                                    });
                                }
                            } catch (error) {
                                console.error(
                                    `Error migrating media for ${langCode}:`,
                                    error,
                                );
                                // Hata olursa boş object oluştur
                                form.value.data[langCode].media = {
                                    cover: [],
                                    icon: [],
                                    video: [],
                                    gallery: [],
                                    document: [],
                                };
                            }
                        });
                    } else {
                        Swal.fire({
                            icon: "error",
                            title: "Sayfa yüklenirken hata oluştu",
                            text: pageData.error || "Bilinmeyen hata",
                            toast: true,
                            position: "top-end",
                            showConfirmButton: false,
                            timer: 7000,
                        });
                    }
                } catch (error) {
                    console.error("loadContent error:", error);
                    // Sadece kritik hatalarda göster (form data yüklenmediyse)
                    if (
                        !form.value.data ||
                        Object.keys(form.value.data).length === 0
                    ) {
                        Swal.fire({
                            icon: "error",
                            title: "Hata!",
                            text:
                                "Sayfa yüklenirken hata oluştu: " +
                                error.message,
                            toast: true,
                            position: "top-end",
                            showConfirmButton: false,
                            timer: 7000,
                        });
                    }
                } finally {
                    loading.value = false;
                }
            };

            // Initialize Media Manager
            const mediaManager = window.createMediaManager(form, contentId);

            // medya yöneticisi metodları ve durumunu ayır
            const {
                uploadingMedia,
                editingMedia,
                editingMediaLang,
                editingMediaIndex,
                editMediaFile,
                libraryMedia,
                libraryMeta,
                loadingLibrary,
                selectedLibraryMedia,
                currentLibraryLang,
                libraryFilters,
                groupedMedia: mediaGroupedMedia,
                computedAutoUrl,
                autoUrlLoading,
                updateComputedAutoUrl,
                applyComputedAutoUrl,
                // New absolute url helpers (term/content)
                copyToClipboard,
                // search bindings
                searchTerm,
                searchResults,
                searchLoading,
                debouncedSearch,
                performSearch,
                selectSearchResult,
                selectedSearchId,
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
                initSortable: initMediaSortable,
                get_media_images_thumbnail,
            } = mediaManager;

            // saveContent: kaydetme işlemi. opsiyonel parametrelerle otomatik kaydetme desteklenir
            // options: { skipValidation: boolean, silent: boolean }
            const saveContent = async (options = {}) => {
                const { skipValidation = false, silent = false } = options;

                // Validation - her dil için başlık ve slug kontrolü (opsiyonel)
                if (!skipValidation) {
                    for (const [langCode, langName] of Object.entries(
                        supportedLanguages.value,
                    )) {
                        if (!form.value.data[langCode]?.title?.trim()) {
                            Swal.fire({
                                icon: "warning",
                                title: `${langName} başlık zorunludur!`,
                                toast: true,
                                position: "top-end",
                                showConfirmButton: false,
                                timer: 5000,
                            });
                            return;
                        }
                        if (!form.value.data[langCode]?.slug?.trim()) {
                            Swal.fire({
                                icon: "warning",
                                title: `${langName} slug zorunludur!`,
                                toast: true,
                                position: "top-end",
                                showConfirmButton: false,
                                timer: 5000,
                            });
                            return;
                        }
                    }
                }

                saving.value = true;
                try {
                    const url = isEditing.value
                        ? `/admin/api/contents/${contentId}`
                        : `/admin/api/contents?content_type=${form.value.content_type}`;

                    const method = isEditing.value ? "PUT" : "POST";

                    // Form data'sını API formatına çevir - Raw JSON olarak gönder
                    const payload = {
                        content_type: form.value.content_type,
                        publish: form.value.publish,
                        gcx: form.value.gcx,
                        parent_id: form.value.parent_id,
                        term_ids: form.value.term_ids,
                        tag_ids: form.value.tag_ids,
                        data: {
                            langs: form.value.data,
                            template: form.value.template,
                            icon: form.value.icon || null,
                            color1: form.value.color1 || null,
                            color2: form.value.color2 || null,
                            color3: form.value.color3 || null,
                            settings: form.value.settings,
                            sub_contents: form.value.sub_contents,
                            term_master_id: form.value.term_master_id, // Ana kategori ID'si data içinde
                            form_settings: form.value.form_settings, // Form ayarları
                        },
                    };

                    if (form.value.product) {
                        // Temizlenmiş product verisini hazırla
                        const cleanedProduct = {
                            ...form.value.product,
                            discount_percentage:
                                form.value.product.discount_percentage === "" ||
                                form.value.product.discount_percentage === undefined
                                    ? 0
                                    : form.value.product.discount_percentage,
                            variants: form.value.product.variants.map(
                                (variant) => ({
                                    ...variant,
                                    discount_percentage:
                                        variant.discount_percentage === "" ||
                                        variant.discount_percentage === undefined
                                            ? 0
                                            : variant.discount_percentage,
                                    price:
                                        variant.price === "" ||
                                        variant.price === undefined
                                            ? null
                                            : variant.price,
                                    b2b_price:
                                        variant.b2b_price === "" ||
                                        variant.b2b_price === undefined
                                            ? null
                                            : variant.b2b_price,
                                }),
                            ),
                        };
                        payload.data.product = cleanedProduct;
                    }

                    const response = await fetch(url, {
                        method: method,
                        headers: {
                            "Content-Type": "application/json",
                        },
                        body: JSON.stringify(payload),
                    });

                    const data = await response.json();

                    if (response.ok) {
                        // If creating new page, redirect to edit mode
                        if (!isEditing.value && data.id) {
                            if (!silent) {
                                Swal.fire({
                                    icon: "success",
                                    title: "Sayfa başarıyla oluşturuldu!",
                                    toast: true,
                                    position: "top-end",
                                    showConfirmButton: false,
                                    timer: 4000,
                                }).then(() => {
                                    window.location.href = `/admin/contents/${form.value.content_type}/${data.id}`;
                                });
                            } else if (data.id) {
                                // still redirect if silent (creation)
                                window.location.href = `/admin/contents/${form.value.content_type}/${data.id}`;
                            }
                        } else {
                            // If editing, just show success message and stay on page
                            if (!silent) {
                                Swal.fire({
                                    icon: "success",
                                    title: "Sayfa başarıyla güncellendi!",
                                    toast: true,
                                    position: "top-end",
                                    showConfirmButton: false,
                                    timer: 4000,
                                });
                            }
                        }
                    } else {
                        Swal.fire({
                            icon: "error",
                            title: "Hata!",
                            text:
                                "Sayfa kaydedilirken hata oluştu 2: " +
                                data.error,
                            toast: true,
                            position: "top-end",
                            showConfirmButton: false,
                            timer: 7000,
                        });
                    }
                } catch (error) {
                    Swal.fire({
                        icon: "error",
                        title: "Sayfa kaydedilirken hata oluştu",
                        text: error.message,
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 7000,
                    });
                } finally {
                    saving.value = false;
                }
            };

            // Parent sayfa bilgisini yükle
            const loadParentPage = async () => {
                if (!form.value.parent_id) {
                    parentPage.value = null;
                    pageBreadcrumbs.value = [];
                    return;
                }

                try {
                    const response = await fetch(
                        `/admin/api/contents/${form.value.parent_id}`,
                    );
                    const data = await response.json();

                    if (response.ok) {
                        parentPage.value = data;
                        // Breadcrumb'ı sadece henüz set edilmemişse al
                        // (loadContent zaten set etmiş olabilir)
                        if (
                            pageBreadcrumbs.value.length === 0 &&
                            data.breadcrumbs &&
                            Array.isArray(data.breadcrumbs)
                        ) {
                            pageBreadcrumbs.value = data.breadcrumbs;
                        }
                    }
                } catch (error) {
                    console.error("Parent sayfa yüklenirken hata:", error);
                }
            };

            // Mevcut parent sayfaları yükle
            const loadAvailableParents = async (search = "") => {
                loadingParents.value = true;
                try {
                    const params = new URLSearchParams({
                        page: 1,
                        limit: 100,
                        content_type: form.value.content_type, // Mevcut içerik türüne göre filtrele
                    });

                    if (search) {
                        params.set("search", search);
                    }

                    const response = await fetch(
                        `/admin/api/contents?${params}`,
                    );
                    const data = await response.json();

                    if (response.ok) {
                        // Düzenleme modundaysa, mevcut içeriği listeden çıkar (kendini parent yapamaz)
                        let pages = data && data.data ? data.data : [];
                        if (isEditing.value) {
                            pages = pages.filter(
                                (p) => p.id !== parseInt(contentId),
                            );
                        }
                        availableParentPagesRaw.value = pages;
                    } else {
                        console.error(
                            "Parent içerikler yüklenirken hata:",
                            data,
                        );
                    }
                } catch (error) {
                    console.error("Parent içerikler yüklenirken hata:", error);
                } finally {
                    loadingParents.value = false;
                }
            };

            // Parent sayfa ara (debounce)
            const searchParentPages = () => {
                clearTimeout(parentSearchTimeout.value);
                parentSearchTimeout.value = setTimeout(() => {
                    loadAvailableParents(parentSearch.value);
                }, 300);
            };

            // Modal'ı göster
            const showParentModal = () => {
                parentSearch.value = "";
                loadAvailableParents();

                if (!parentModal) {
                    const modalElement =
                        document.getElementById("parentPageModal");
                    parentModal = new bootstrap.Modal(modalElement);
                }
                parentModal.show();
            };

            // Otomatik kaydetme: medya güncelleme gibi küçük değişikliklerde otomatik içerik kaydet
            let autoSaveTimeout = null;
            const scheduleAutoSave = (delay = 800) => {
                clearTimeout(autoSaveTimeout);
                autoSaveTimeout = setTimeout(() => {
                    // silent ve skipValidation ile otomatik ve sessiz kaydet
                    saveContent({ skipValidation: true, silent: true });
                }, delay);
            };

            document.addEventListener("content-media-updated", (e) => {
                // e.detail may contain lang
                scheduleAutoSave(800);
            });

            // Parent seç
            const selectParent = (page) => {
                form.value.parent_id = page.id;

                // Parent page objesini doğru formatta oluştur
                parentPage.value = {
                    id: page.id,
                    langs: page.data?.langs || {},
                };

                parentModal.hide();

                const title = getPageTitle(page);
                Swal.fire({
                    icon: "success",
                    title: `"${title}" üst sayfa olarak ayarlandı`,
                    toast: true,
                    position: "top-end",
                    showConfirmButton: false,
                    timer: 4000,
                });
            };

            // Helper fonksiyonlar - Raw JSON data parse
            const getPageTitle = (page) => {
                if (!page || !page.data) return "Başlıksız";

                const lang = "tr";

                // Yeni format: data.langs.tr.title
                if (
                    page.data.langs &&
                    page.data.langs[lang] &&
                    page.data.langs[lang].title
                ) {
                    return page.data.langs[lang].title;
                }

                // Alternatif format: data.tr.title
                if (page.data[lang] && page.data[lang].title) {
                    return page.data[lang].title;
                }

                return "Başlıksız";
            };

            const getPageSlug = (page) => {
                if (!page || !page.data) return "";

                const lang = "tr";

                // Yeni format: data.langs.tr.slug
                if (
                    page.data.langs &&
                    page.data.langs[lang] &&
                    page.data.langs[lang].slug
                ) {
                    return page.data.langs[lang].slug;
                }

                // Alternatif format: data.tr.slug
                if (page.data[lang] && page.data[lang].slug) {
                    return page.data[lang].slug;
                }

                return "";
            };

            // Parent'ı kaldır
            const removeParent = () => {
                Swal.fire({
                    title: "Emin misiniz?",
                    text: "Üst sayfa ilişkisi kaldırılacak",
                    icon: "warning",
                    showCancelButton: true,
                    confirmButtonText: "Evet, Kaldır",
                    cancelButtonText: "İptal",
                }).then((result) => {
                    if (result.isConfirmed) {
                        form.value.parent_id = null;
                        parentPage.value = null;

                        Swal.fire({
                            icon: "success",
                            title: "Kaldırıldı",
                            text: "Üst sayfa ilişkisi kaldırıldı",
                            toast: true,
                            position: "top-end",
                            showConfirmButton: false,
                            timer: 4000,
                        });
                    }
                });
            };

            // Kategori fonksiyonları

            // Term ve tag ID'lerini ayır (edit modunda kullanılır)
            const separateTermsAndTags = () => {
                if (!form.value._allTermIds) return;

                const allTermIds = form.value._allTermIds;

                // Tag ID'lerini al
                const tagIds = availableTags.value.map((t) => t.id);

                // Term ID'lerini ayır (tag olmayanlar)
                form.value.term_ids = allTermIds.filter(
                    (id) => !tagIds.includes(id),
                );
                form.value.tag_ids = allTermIds.filter((id) =>
                    tagIds.includes(id),
                );

                // Seçili tag'leri set et
                selectedTags.value = availableTags.value.filter((tag) =>
                    form.value.tag_ids.includes(tag.id),
                );

                // Geçici alanı temizle
                delete form.value._allTermIds;
            };

            const loadTerms = async () => {
                const term_type = form.value.content_type;
                loadingTerms.value = true;
                try {
                    const response = await fetch(
                        `/admin/api/content-types/${term_type}/terms?ghost=true`,
                    );
                    const data = await response.json();
                    if (response.ok) {
                        availableTerms.value = data || [];
                    }
                } catch (error) {
                    console.error("Term'ler yüklenirken hata:", error);
                } finally {
                    loadingTerms.value = false;
                }
            };

            // Content type değiştiğinde
            const onContentTypeChange = () => {
                // Parent'ı temizle (farklı türde içerik parent olamaz)
                form.value.parent_id = null;
                parentPage.value = null;

                // Kategorileri ve tag'leri temizle
                form.value.term_master_id = null;
                form.value.term_ids = [];
                form.value.tag_ids = [];
                selectedTags.value = [];

                // Yeni content type'a göre kategorileri yükle
                loadTerms();

                // Eğer product ise vocabulary'leri yükle
                if (form.value.content_type === "product") {
                    loadAvailableVocabularies();
                }
            };

            const toggleTerm = (termId) => {
                const currentIds = [...form.value.term_ids]; // Kopya oluştur
                const index = currentIds.indexOf(termId);

                if (index > -1) {
                    currentIds.splice(index, 1);
                } else {
                    currentIds.push(termId);
                }

                form.value.term_ids = currentIds; // Yeni array ata (Vue reactivity için)
            };

            const getTermTitle = (termId) => {
                const findTermRecursive = (terms, id) => {
                    for (const term of terms) {
                        if (term.id === id) return term.title;
                        if (term.children && term.children.length > 0) {
                            const found = findTermRecursive(term.children, id);
                            if (found) return found;
                        }
                    }
                    return null;
                };
                return findTermRecursive(availableTerms.value, termId) || "";
            };

            // ============ PRODUCT ATTRIBUTES ============

            // Vocabulary & Attribute state
            const availableVocabularies = ref([]);
            const selectedVocabularyIds = ref([]);
            const selectedVocabularies = ref([]);
            const loadingVocabularies = ref(false);
            const loadingAttributes = ref(false);
            const newAttributeValues = ref({});
            const showAddNewAttributeValue = ref({});
            const showAttributeSuggestions = ref({});
            const attributeSuggestions = ref({});

            // Mevcut vocabulary'leri yükle
            const loadAvailableVocabularies = async () => {
                loadingVocabularies.value = true;
                try {
                    const response = await fetch(
                        "/admin/api/vocabularies?type=product_attributes",
                    );
                    const data = await response.json();

                    if (response.ok && data.success) {
                        availableVocabularies.value = data.data || [];
                        console.log(
                            `${availableVocabularies.value.length} özellik grubu yüklendi`,
                        );
                    } else {
                        throw new Error(data.error || "Bilinmeyen hata");
                    }
                } catch (error) {
                    console.error("Vocabulary'ler yüklenirken hata:", error);
                    Swal.fire({
                        icon: "error",
                        title: "Özellik grupları yüklenirken hata oluştu",
                        text: error.message,
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 7000,
                    });
                } finally {
                    loadingVocabularies.value = false;
                }
            };

            // Vocabulary seçimi değiştiğinde
            const onVocabularySelectionChange = async () => {
                // Seçili vocabulary'leri güncelle
                selectedVocabularies.value = availableVocabularies.value.filter(
                    (vocab) => selectedVocabularyIds.value.includes(vocab.id),
                );

                // Seçili vocabulary'ler için attribute'ları yükle
                if (selectedVocabularies.value.length > 0) {
                    await loadSelectedVocabularyAttributes();
                }
            };

            // Seçili vocabulary'lerin attribute'larını yükle
            const loadSelectedVocabularyAttributes = async () => {
                loadingAttributes.value = true;
                try {
                    // Her seçili vocabulary için term'leri yükle
                    for (const vocab of selectedVocabularies.value) {
                        const termsResponse = await fetch(
                            `/admin/api/vocabularies/${vocab.id}/terms`,
                        );
                        const termsData = await termsResponse.json();

                        if (termsResponse.ok && termsData.success) {
                            const terms = termsData.data?.data || [];

                            // Vocabulary data'sından bilgileri al
                            const vocabData = vocab.data;
                            const attrName =
                                vocabData.name || `attr_${vocab.id}`;
                            const attrRequired = vocabData.required || false;

                            // Vocabulary objesine attribute bilgilerini ekle
                            vocab.name = attrName;
                            vocab.required = attrRequired;
                            vocab.values = terms.map((term) => {
                                const value =
                                    term.data.value ||
                                    term.data.langs?.tr?.title
                                        ?.toLowerCase()
                                        .replace(/\s+/g, "_") ||
                                    "";
                                const displayValue =
                                    term.data.display_value ||
                                    term.data.langs?.tr?.title ||
                                    term.title ||
                                    value;

                                console.log(
                                    `Mapping term ${term.id}: value="${value}", display_value="${displayValue}"`,
                                );

                                return {
                                    id: term.id,
                                    value: value,
                                    display_value: displayValue,
                                };
                            });

                            // Form'da bu attribute için array oluştur
                            if (!form.value.product.attributes) {
                                form.value.product.attributes = {};
                            }

                            if (!form.value.product.attributes[attrName]) {
                                form.value.product.attributes[attrName] = [];
                            }

                            // Eğer eski format string ise array'e çevir
                            if (
                                typeof form.value.product.attributes[
                                    attrName
                                ] === "string"
                            ) {
                                form.value.product.attributes[attrName] = form
                                    .value.product.attributes[attrName]
                                    ? [form.value.product.attributes[attrName]]
                                    : [];
                            }

                            // Array değilse boş array yap
                            if (
                                !Array.isArray(
                                    form.value.product.attributes[attrName],
                                )
                            ) {
                                form.value.product.attributes[attrName] = [];
                            }
                        }
                    }

                    console.log(
                        `${selectedVocabularies.value.length} özellik grubu için değerler yüklendi`,
                    );

                    // Geçersiz değerleri temizle
                    cleanupInvalidAttributeValues();
                } catch (error) {
                    console.error(
                        "Attribute değerleri yüklenirken hata:",
                        error,
                    );
                    Swal.fire({
                        icon: "error",
                        title: "Özellik değerleri yüklenirken hata oluştu",
                        text: error.message,
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 7000,
                    });
                } finally {
                    loadingAttributes.value = false;
                }
            };

            // Mevcut ürün için seçili vocabulary'leri restore et
            const restoreSelectedVocabularies = async () => {
                if (!form.value.product.attributes) return;

                // Mevcut attribute'lara sahip vocabulary'leri bul
                const usedVocabularyNames = Object.keys(
                    form.value.product.attributes,
                ).filter(
                    (attrName) =>
                        form.value.product.attributes[attrName] &&
                        Array.isArray(
                            form.value.product.attributes[attrName],
                        ) &&
                        form.value.product.attributes[attrName].length > 0,
                );

                // Bu attribute name'lere sahip vocabulary'leri seç
                const vocabularyIds = [];
                for (const vocab of availableVocabularies.value) {
                    const vocabName = vocab.data.name || `attr_${vocab.id}`;
                    if (usedVocabularyNames.includes(vocabName)) {
                        vocabularyIds.push(vocab.id);
                    }
                }

                selectedVocabularyIds.value = vocabularyIds;
                await onVocabularySelectionChange();
            };

            // Ana kategori değiştiğinde çağrılır (artık attribute'ları etkilemez)
            const onMasterCategoryChange = () => {
                // Artık attribute'lar kategori bağımsız, bu fonksiyon boş
                console.log(
                    "Master category changed, but attributes are now category-independent",
                );
            };

            // Attribute değiştiğinde çağrılır
            const onAttributeChange = (vocabulary) => {
                console.log(
                    `Attribute ${vocabulary.name} changed to:`,
                    form.value.product.attributes[vocabulary.name],
                );
            };

            // Yeni attribute değeri ekle
            const addNewAttributeValue = async (attribute) => {
                const newValue =
                    newAttributeValues.value[attribute.name]?.trim();
                if (!newValue) return;

                try {
                    const payload = {
                        vocabulary_id: attribute.id,
                        data: {
                            langs: {
                                tr: {
                                    title: newValue,
                                    description: "",
                                },
                                en: {
                                    title: newValue,
                                    description: "",
                                },
                            },
                        },
                        publish: true,
                    };

                    console.log("Creating new term with payload:", payload);

                    const response = await fetch(
                        `/admin/api/vocabularies/${attribute.id}/terms`,
                        {
                            method: "POST",
                            headers: {
                                "Content-Type": "application/json",
                            },
                            body: JSON.stringify(payload),
                        },
                    );

                    if (response.ok) {
                        const result = await response.json();
                        console.log("Term creation response:", result);

                        if (result.success && result.data) {
                            // Attribute'ın values listesine ekle
                            attribute.values.push({
                                id: result.data.id,
                                title: newValue,
                                display_value: newValue,
                            });

                            // Form'da bu değeri seç (array'e ekle)
                            if (
                                !form.value.product.attributes[attribute.name]
                            ) {
                                form.value.product.attributes[attribute.name] =
                                    [];
                            }
                            form.value.product.attributes[attribute.name].push(
                                result.data.id,
                            );

                            // Input'u temizle
                            newAttributeValues.value[attribute.name] = "";

                            Swal.fire({
                                icon: "success",
                                title: `"${newValue}" değeri ${attribute.title} listesine eklendi`,
                                toast: true,
                                position: "top-end",
                                showConfirmButton: false,
                                timer: 4000,
                            });
                        } else {
                            throw new Error(
                                result.error || "Değer oluşturulamadı",
                            );
                        }
                    } else {
                        const result = await response.json();
                        throw new Error(result.error || "Değer oluşturulamadı");
                    }
                } catch (error) {
                    console.error("Yeni değer eklenirken hata:", error);
                    Swal.fire({
                        icon: "error",
                        title: "Yeni değer eklenirken hata oluştu",
                        text: error.message,
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 7000,
                    });
                }
            };

            // Attribute değer başlığını getir
            const getAttributeValueTitle = (attribute, valueId) => {
                if (!attribute || !attribute.values) {
                    console.warn(
                        "getAttributeValueTitle: attribute or values is missing",
                        attribute,
                    );
                    return "Bilinmeyen";
                }

                const value = attribute.values.find((v) => v.id === valueId);
                if (!value) {
                    console.warn(
                        `getAttributeValueTitle: value not found for ID ${valueId} in`,
                        attribute.values,
                    );
                }

                return value ? value.display_value : "Bilinmeyen";
            };

            // Attribute değerini kaldır (badge'e tıklayınca)
            const removeAttributeValue = (vocabulary, valueId) => {
                if (!form.value.product.attributes[vocabulary.name]) return;

                // Array'den değeri kaldır
                const currentValues =
                    form.value.product.attributes[vocabulary.name];
                const newValues = currentValues.filter((id) => id !== valueId);
                form.value.product.attributes[vocabulary.name] = newValues;

                // İlgili checkbox'ı da uncheck yap
                const checkbox = document.getElementById(
                    `attr_${vocabulary.name}_${valueId}`,
                );
                if (checkbox) {
                    checkbox.checked = false;
                }

                console.log(`Removed value ${valueId} from ${vocabulary.name}`);
            };

            // Geçersiz attribute değerlerini temizle
            const cleanupInvalidAttributeValues = () => {
                if (!form.value.product.attributes) return;

                let hasChanges = false;

                selectedVocabularies.value.forEach((vocabulary) => {
                    const attrName = vocabulary.name;
                    const currentValues =
                        form.value.product.attributes[attrName];

                    if (
                        Array.isArray(currentValues) &&
                        currentValues.length > 0
                    ) {
                        // Mevcut vocabulary'de bulunan geçerli ID'leri al
                        const validIds = vocabulary.values
                            ? vocabulary.values.map((v) => v.id)
                            : [];

                        // Sadece geçerli ID'leri koru
                        const cleanedValues = currentValues.filter((id) =>
                            validIds.includes(id),
                        );

                        if (cleanedValues.length !== currentValues.length) {
                            form.value.product.attributes[attrName] =
                                cleanedValues;
                            hasChanges = true;
                            console.log(
                                `Cleaned invalid values from ${attrName}: ${currentValues.length} -> ${cleanedValues.length}`,
                            );
                        }
                    }
                });

                if (hasChanges) {
                    console.log("Invalid attribute values cleaned up");
                }
            };

            // Seçili attribute'lar var mı kontrol et
            const hasSelectedAttributes = computed(() => {
                if (!form.value.product.attributes) return false;

                return Object.values(form.value.product.attributes).some(
                    (values) => Array.isArray(values) && values.length > 0,
                );
            });

            // Varyant oluşturma için seçili vocabulary'ler
            const selectedVariantVocabularyIds = ref([]);
            const selectedVocabulariesForVariants = computed(() => {
                return selectedVocabularies.value.filter((vocabulary) => {
                    const attrName = vocabulary.name;
                    const selectedValues =
                        form.value.product.attributes[attrName];
                    return (
                        Array.isArray(selectedValues) &&
                        selectedValues.length > 0
                    );
                });
            });
            let variantGenerationModal = null;

            // Varyant oluşturma modalını göster
            const showVariantGenerationModal = () => {
                if (!hasSelectedAttributes.value) {
                    Swal.fire({
                        icon: "warning",
                        title: "Uyarı!",
                        text: "Varyant oluşturmak için önce özellik değerleri seçmelisiniz.",
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 5000,
                    });
                    return;
                }

                // Modal'ı aç
                if (!variantGenerationModal) {
                    variantGenerationModal = new bootstrap.Modal(
                        document.getElementById("variantGenerationModal"),
                    );
                }

                // Seçimi temizle
                selectedVariantVocabularyIds.value = [];

                variantGenerationModal.show();
            };

            // Seçili attribute sayısını getir
            const getSelectedAttributeCount = (vocabulary) => {
                const attrName = vocabulary.name;
                const selectedValues = form.value.product.attributes[attrName];
                return Array.isArray(selectedValues)
                    ? selectedValues.length
                    : 0;
            };

            // Varyant sayısını hesapla
            const calculateVariantCount = () => {
                if (selectedVariantVocabularyIds.value.length === 0) return 0;

                let totalCombinations = 1;

                selectedVariantVocabularyIds.value.forEach((vocabId) => {
                    const vocabulary =
                        selectedVocabulariesForVariants.value.find(
                            (v) => v.id === vocabId,
                        );
                    if (vocabulary) {
                        const count = getSelectedAttributeCount(vocabulary);
                        if (count > 0) {
                            totalCombinations *= count;
                        }
                    }
                });

                return totalCombinations;
            };

            // attribute'lardan options oluşturup varyant oluştur
            const generateVariantsFromSelectedAttributes = () => {
                if (selectedVariantVocabularyIds.value.length === 0) {
                    Swal.fire({
                        icon: "warning",
                        title: "Uyarı!",
                        text: "Varyant oluşturmak için en az bir özellik grubu seçmelisiniz.",
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 5000,
                    });
                    return;
                }

                // Seçili vocabulary'leri filtrele
                const selectedVocabs =
                    selectedVocabulariesForVariants.value.filter((vocab) =>
                        selectedVariantVocabularyIds.value.includes(vocab.id),
                    );

                // Seçili attribute'ları options formatına çevir
                const attributeOptions = [];

                selectedVocabs.forEach((vocabulary) => {
                    const attrName = vocabulary.name;
                    const selectedValues =
                        form.value.product.attributes[attrName];

                    if (
                        Array.isArray(selectedValues) &&
                        selectedValues.length > 0
                    ) {
                        // Seçili değerlerin display_value'larını al
                        const valueNames = selectedValues
                            .map((valueId) => {
                                const value = vocabulary.values.find(
                                    (v) => v.id === valueId,
                                );
                                return value
                                    ? value.display_value
                                    : `ID:${valueId}`;
                            })
                            .filter((name) => name);

                        if (valueNames.length > 0) {
                            attributeOptions.push({
                                name: vocabulary.title, // Vocabulary title'ını option name olarak kullan
                                values: valueNames.join(", "), // Değerleri virgülle ayır
                                position: attributeOptions.length,
                            });
                        }
                    }
                });

                if (attributeOptions.length === 0) {
                    Swal.fire({
                        icon: "warning",
                        title: "Uyarı!",
                        text: "Seçili gruplarda özellik değeri bulunamadı.",
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 5000,
                    });
                    return;
                }

                // Options'ları tamamen temizle ve sadece seçili attribute'lardan oluştur
                // Bu sayede duplicate'ler ve eski options'lar temizlenir
                form.value.product.options = [...attributeOptions];

                console.log(
                    "Options cleared and recreated from attributes:",
                    attributeOptions,
                );

                // Options hazırlandı, şimdi varyantları oluştur
                generateVariants();

                // Modal'ı kapat
                variantGenerationModal.hide();

                // Başarı mesajı göster
                const totalCombinations = form.value.product.variants
                    ? form.value.product.variants.length
                    : 0;
                const selectedGroupNames = selectedVocabs
                    .map((v) => v.title)
                    .join(", ");

                Swal.fire({
                    icon: "success",
                    title: "Varyantlar Oluşturuldu!",
                    text: `${selectedGroupNames} gruplarından ${totalCombinations} varyant oluşturuldu.`,
                    toast: true,
                    position: "top-end",
                    showConfirmButton: false,
                    timer: 4000,
                });

                console.log(
                    `Generated ${totalCombinations} variants from selected groups:`,
                    selectedVocabs.map((v) => v.title),
                );
            };

            // Text input için önerileri ara
            const searchAttributeValues = async (attribute, searchTerm) => {
                if (!searchTerm || searchTerm.length < 2) {
                    attributeSuggestions.value[attribute.name] = [];
                    return;
                }

                // Mevcut değerler arasında ara
                const filtered = attribute.values.filter((value) =>
                    value.display_value
                        .toLowerCase()
                        .includes(searchTerm.toLowerCase()),
                );

                attributeSuggestions.value[attribute.name] = filtered.slice(
                    0,
                    5,
                );
            };

            // Öneri seç
            const selectAttributeSuggestion = (attribute, suggestion) => {
                form.value.product.attributes[attribute.name] =
                    suggestion.value;
                showAttributeSuggestions.value[attribute.name] = false;
            };

            // Önerileri gizle (delay ile)
            const hideAttributeSuggestions = (attributeName) => {
                setTimeout(() => {
                    showAttributeSuggestions.value[attributeName] = false;
                }, 200);
            };

            // Tag fonksiyonları
            const getTagTitle = (tag) => {
                // Yeni format: data.langs.tr.title
                if (tag.data?.langs?.tr?.title) {
                    return tag.data.langs.tr.title;
                }
                // Eski format: data.tr.title (geriye dönük uyumluluk)
                if (tag.data?.tr?.title) {
                    return tag.data.tr.title;
                }
                // Fallback: direkt title varsa
                if (tag.title) {
                    return tag.title;
                }
                return "Untitled";
            };

            const loadTags = async () => {
                loadingTags.value = true;
                try {
                    // Vocabulary ID 4 için tag'leri yükle, daha sonra bu 4 otomaik olacak şimdilik böyle manuel
                    const response = await fetch(
                        "/admin/api/vocabularies/4/terms",
                    );
                    const result = await response.json();

                    if (response.ok && result.success) {
                        // data.data  daha sonra değişecek  böyle saçma oldu, data.results  olacak
                        availableTags.value = result.data?.data || [];
                    } else {
                        availableTags.value = [];
                    }
                } catch (error) {
                    console.error("Tag'ler yüklenirken hata:", error);
                } finally {
                    loadingTags.value = false;
                }
            };

            const searchTags = () => {
                clearTimeout(tagSearchTimeout.value);
                tagSearchTimeout.value = setTimeout(() => {
                    const search = tagSearch.value.toLowerCase().trim();
                    if (search.length === 0) {
                        tagSuggestions.value = [];
                        return;
                    }

                    // availableTags array kontrolü
                    if (!Array.isArray(availableTags.value)) {
                        tagSuggestions.value = [];
                        return;
                    }

                    // Zaten seçili olmayan tag'leri filtrele
                    const selectedIds = selectedTags.value.map((t) => t.id);
                    tagSuggestions.value = availableTags.value
                        .filter(
                            (tag) =>
                                !selectedIds.includes(tag.id) &&
                                tag.title.toLowerCase().includes(search),
                        )
                        .slice(0, 10);

                    selectedTagIndex.value = 0;
                }, 300);
            };

            const navigateTagSuggestions = (direction) => {
                if (tagSuggestions.value.length === 0) return;

                selectedTagIndex.value += direction;
                if (selectedTagIndex.value < 0) {
                    selectedTagIndex.value = tagSuggestions.value.length - 1;
                } else if (
                    selectedTagIndex.value >= tagSuggestions.value.length
                ) {
                    selectedTagIndex.value = 0;
                }
            };

            const addTag = (tag) => {
                if (!selectedTags.value.find((t) => t.id === tag.id)) {
                    selectedTags.value.push(tag);
                    form.value.tag_ids = selectedTags.value.map((t) => t.id);
                }
                tagSearch.value = "";
                tagSuggestions.value = [];
            };

            const addTagFromInput = async () => {
                const search = tagSearch.value.trim();
                if (!search) return;

                // Önce suggestion'lardan seç
                if (tagSuggestions.value.length > 0) {
                    addTag(tagSuggestions.value[selectedTagIndex.value]);
                    return;
                }

                // Yeni tag oluştur
                try {
                    const response = await fetch(
                        "/admin/api/vocabularies/4/terms",
                        {
                            method: "POST",
                            headers: {
                                "Content-Type": "application/json",
                            },
                            body: JSON.stringify({
                                vocabulary_id: 4,
                                data: {
                                    langs: {
                                        tr: {
                                            title: search,
                                            description: "",
                                        },
                                        en: {
                                            title: search,
                                            description: "",
                                        },
                                    },
                                },
                                parent_id: null,
                                publish: true,
                            }),
                        },
                    );

                    if (response.ok) {
                        const result = await response.json();
                        if (result.success && result.data) {
                            availableTags.value.push(result.data);
                            addTag(result.data);

                            Swal.fire({
                                icon: "success",
                                title: "Yeni Etiket!",
                                text: `"${search}" etiketi oluşturuldu.`,
                                toast: true,
                                position: "top-end",
                                showConfirmButton: false,
                                timer: 4000,
                            });
                        } else {
                            throw new Error(
                                result.error || "Tag oluşturulamadı",
                            );
                        }
                    } else {
                        const result = await response.json();
                        throw new Error(result.error || "Tag oluşturulamadı");
                    }
                } catch (error) {
                    console.error("Tag oluşturulurken hata:", error);
                    Swal.fire({
                        icon: "error",
                        title: "Hata!",
                        text: "Etiket oluşturulamadı.",
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 7000,
                    });
                }
            };

            const removeTag = (tagId) => {
                selectedTags.value = selectedTags.value.filter(
                    (t) => t.id !== tagId,
                );
                form.value.tag_ids = selectedTags.value.map((t) => t.id);
            };

            // Load languages from API
            const loadLanguages = async () => {
                loadingLanguages.value = true;
                try {
                    const response = await fetch("/admin/api/languages");
                    const data = await response.json();
                    if (response.ok) {
                        // API'den gelen map'i direkt kullan
                        supportedLanguages.value = data.supported_languages;
                        defaultLanguage.value = data.default_language || "tr";

                        // Form data'yı dillere göre initialize et
                        const formData = {};
                        Object.keys(data.supported_languages).forEach(
                            (langCode) => {
                                formData[langCode] = {
                                    title: "",
                                    slug: "",
                                    description: "",
                                    body: "",
                                    meta_title: "",
                                    meta_description: "",
                                    media: {
                                        cover: [],
                                        icon: [],
                                        video: [],
                                        gallery: [],
                                        document: [],
                                    },
                                };
                            },
                        );
                        form.value.data = formData;

                        // Form settings'i initialize et (eğer yoksa)
                        if (!form.value.form_settings) {
                            form.value.form_settings = {
                                allowAnonymous: true,
                                sendEmail: false,
                                fields: [],
                            };
                        }
                    }
                } catch (error) {
                    console.error("Error loading languages:", error);
                    // Fallback to default languages
                    supportedLanguages.value = {
                        tr: "Türkçe",
                        en: "English",
                        de: "Deutsch",
                    };
                    // Fallback form data
                    form.value.data = {
                        tr: {
                            title: "",
                            slug: "",
                            description: "",
                            body: "",
                            meta_title: "",
                            meta_description: "",
                            media: {
                                cover: [],
                                icon: [],
                                video: [],
                                gallery: [],
                                document: [],
                            },
                        },
                        en: {
                            title: "",
                            slug: "",
                            description: "",
                            body: "",
                            meta_title: "",
                            meta_description: "",
                            media: {
                                cover: [],
                                icon: [],
                                video: [],
                                gallery: [],
                                document: [],
                            },
                        },
                        de: {
                            title: "",
                            slug: "",
                            description: "",
                            body: "",
                            meta_title: "",
                            meta_description: "",
                            media: {
                                cover: [],
                                icon: [],
                                video: [],
                                gallery: [],
                                document: [],
                            },
                        },
                    };

                    // Form settings'i initialize et (eğer yoksa)
                    if (!form.value.form_settings) {
                        form.value.form_settings = {
                            allowAnonymous: true,
                            sendEmail: false,
                            fields: [],
                        };
                    }
                } finally {
                    loadingLanguages.value = false;
                }
            };

            const loadTemplates = async () => {
                try {
                    const response = await fetch("/admin/api/templates");
                    if (response.ok) {
                        availableTemplates.value = await response.json();
                    }
                } catch (e) {
                    console.error("Şablonlar yüklenirken hata oluştu:", e);
                }
            };

            const addSubContent = () => {
                form.value.sub_contents.push({
                    name: "",
                    content_id: null,
                    description: "",
                });
            };

            const removeSubContent = (index) => {
                form.value.sub_contents.splice(index, 1);
            };

            // ürün varyant metodları
            const addProductOption = () => {
                if (!form.value.product.options) {
                    form.value.product.options = [];
                }
                form.value.product.options.push({
                    name: "",
                    values: "",
                    position: form.value.product.options.length,
                });
            };

            const removeProductOption = (index) => {
                form.value.product.options.splice(index, 1);
            };

            const clearProductOptions = () => {
                Swal.fire({
                    title: "Emin misiniz?",
                    text: "Tüm ürün seçenekleri silinecek!",
                    icon: "warning",
                    showCancelButton: true,
                    confirmButtonText: "Evet, Temizle",
                    cancelButtonText: "İptal",
                }).then((result) => {
                    if (result.isConfirmed) {
                        form.value.product.options = [];
                        Swal.fire({
                            icon: "success",
                            title: "Temizlendi!",
                            text: "Tüm ürün seçenekleri silindi.",
                            toast: true,
                            position: "top-end",
                            showConfirmButton: false,
                            timer: 4000,
                        });
                    }
                });
            };

            const addVariant = () => {
                if (!form.value.product.variants) {
                    form.value.product.variants = [];
                }
                form.value.product.variants.push({
                    sku: "",
                    barcode: "",
                    price: form.value.product.price || 0,
                    discount_percentage: 0,
                    stock: 0,
                    weight: null,
                    option_values_display: "",
                    option_values: {},
                    is_active: true,
                    media: {
                        cover: [],
                        gallery: [],
                        icon: [],
                        video: [],
                        document: [],
                    },
                });
            };

            const removeVariant = (index) => {
                form.value.product.variants.splice(index, 1);
            };

            // mevcut options'lardan varyantları oluştur (hem manuel hem attribute'lardan gelen options'lar için)
            const generateVariants = () => {
                if (
                    !form.value.product.options ||
                    form.value.product.options.length === 0
                ) {
                    return;
                }

                console.log(
                    "Form product options:",
                    form.value.product.options,
                );

                // Group options by name and combine their values (duplicate'leri temizle)
                const optionGroups = {};
                form.value.product.options.forEach((opt) => {
                    if (!opt.name || !opt.values) return;

                    const values = opt.values
                        .split(",")
                        .map((v) => v.trim())
                        .filter((v) => v);

                    if (!optionGroups[opt.name]) {
                        optionGroups[opt.name] = [];
                    }
                    // Sadece yeni değerleri ekle (duplicate'leri önle)
                    values.forEach((value) => {
                        if (!optionGroups[opt.name].includes(value)) {
                            optionGroups[opt.name].push(value);
                        }
                    });
                });

                // tekrarları kaldır ve seçenek setleri oluştur
                const optionSets = Object.entries(optionGroups).map(
                    ([name, values]) => {
                        const uniqueValues = [...new Set(values)];
                        console.log(
                            `Option "${name}" has values:`,
                            uniqueValues,
                        );
                        return {
                            name: name,
                            values: uniqueValues,
                        };
                    },
                );

                console.log("Option sets:", optionSets);

                // Generate all combinations
                const generateCombinations = (
                    sets,
                    index = 0,
                    current = {},
                ) => {
                    if (index === sets.length) {
                        return [current];
                    }

                    const results = [];
                    const currentSet = sets[index];

                    for (const value of currentSet.values) {
                        const newCombination = {
                            ...current,
                            [currentSet.name]: value,
                        };
                        results.push(
                            ...generateCombinations(
                                sets,
                                index + 1,
                                newCombination,
                            ),
                        );
                    }

                    return results;
                };

                const combinations = generateCombinations(optionSets);
                console.log("Generated combinations:", combinations);

                // yardımcı fonksiyon: iki option_values objesini karşılaştır
                const compareOptionValues = (obj1, obj2) => {
                    if (!obj1 || !obj2) return false;

                    const keys1 = Object.keys(obj1).sort();
                    const keys2 = Object.keys(obj2).sort();

                    // Key sayıları farklıysa false
                    if (keys1.length !== keys2.length) return false;

                    // Her key ve value'yu karşılaştır
                    for (let i = 0; i < keys1.length; i++) {
                        if (keys1[i] !== keys2[i]) return false;
                        if (obj1[keys1[i]] !== obj2[keys2[i]]) return false;
                    }

                    return true;
                };

                // yardımcı fonksiyon: option_values_display'i normalize et (sıralama ve boşluklar için)
                const normalizeDisplay = (display) => {
                    if (!display) return "";
                    return display
                        .split(" / ")
                        .map((s) => s.trim())
                        .sort()
                        .join(" / ");
                };

                // Mevcut varyantları koru ve yeni olanları ekle
                const existingVariants = form.value.product.variants || [];
                const newVariants = [];

                combinations.forEach((combo, idx) => {
                    const display = Object.entries(combo)
                        .map(([k, v]) => v)
                        .join(" / ");
                    const normalizedDisplay = normalizeDisplay(display);

                    // Bu kombinasyon zaten var mı kontrol et - 3 farklı yöntemle
                    const existingVariant = existingVariants.find((variant) => {
                        // 1. Önce option_values objesini karşılaştır (en güvenilir)
                        if (
                            variant.option_values &&
                            compareOptionValues(variant.option_values, combo)
                        ) {
                            return true;
                        }

                        // 2. Normalize edilmiş display string'leri karşılaştır
                        const variantNormalizedDisplay = normalizeDisplay(
                            variant.option_values_display,
                        );
                        if (variantNormalizedDisplay === normalizedDisplay) {
                            return true;
                        }

                        // 3. Direkt display string karşılaştırması (eski yöntem)
                        if (variant.option_values_display === display) {
                            return true;
                        }

                        return false;
                    });

                    if (existingVariant) {
                        // Mevcut varyant varsa, option_values'ı güncelle (yeni format için)
                        existingVariant.option_values = combo;
                        // Display'i de standart formata getir
                        existingVariant.option_values_display = display;
                        newVariants.push(existingVariant);
                        console.log(
                            `Existing variant preserved: ${display} (was: ${existingVariant.option_values_display})`,
                        );
                    } else {
                        // yeni varyant oluştur
                        const newVariant = {
                            sku: "", // sku boş bırak, kullanıcı manuel dolduracak
                            price: form.value.product.price || 0,
                            b2b_price: form.value.product.b2b_price || null,
                            discount_percentage: form.value.product.discount_percentage || 0,
                            stock: 0,
                            option_values_display: display,
                            option_values: combo,
                            is_active: true,
                            media: {
                                cover: [],
                                gallery: [],
                                icon: [],
                                video: [],
                                document: [],
                            },
                        };
                        newVariants.push(newVariant);
                        console.log(`New variant created: ${display}`);
                    }
                });

                form.value.product.variants = newVariants;

                // Sonuçları logla
                const preservedCount = newVariants.filter((v) =>
                    existingVariants.some(
                        (ev) =>
                            ev.option_values_display ===
                            v.option_values_display,
                    ),
                ).length;
                const createdCount = newVariants.length - preservedCount;

                console.log(
                    `Variants summary: ${preservedCount} preserved, ${createdCount} newly created, ${newVariants.length} total`,
                );

                // Kullanıcıya bilgi ver
                if (preservedCount > 0 && createdCount > 0) {
                    Swal.fire({
                        icon: "info",
                        title: "Varyantlar Güncellendi!",
                        text: `${preservedCount} mevcut varyant korundu, ${createdCount} yeni varyant eklendi.`,
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 5000,
                    });
                } else if (preservedCount > 0 && createdCount === 0) {
                    Swal.fire({
                        icon: "info",
                        title: "Varyantlar Zaten Mevcut!",
                        text: `Tüm varyantlar (${preservedCount} adet) zaten mevcut. Hiçbir değişiklik yapılmadı.`,
                        toast: true,
                        position: "top-end",
                        showConfirmButton: false,
                        timer: 5000,
                    });
                }
            };

            // varyant medya fonksiyonları
            const openVariantMediaModal = (variantIndex) => {
                editingVariant.value = variantIndex;

                // Initialize media structure if not exists
                if (!form.value.product.variants[variantIndex].media) {
                    form.value.product.variants[variantIndex].media = {
                        cover: [],
                        gallery: [],
                        icon: [],
                        video: [],
                        document: [],
                    };
                }

                if (!variantMediaModal) {
                    variantMediaModal = new bootstrap.Modal(
                        document.getElementById("variantMediaModal"),
                    );
                }
                variantMediaModal.show();
            };

            const handleVariantMediaUpload = async (event, variantIndex) => {
                const files = event.target.files;
                if (!files || files.length === 0) return;

                uploadingVariantMedia.value = true;

                try {
                    const formData = new FormData();
                    Array.from(files).forEach((file) =>
                        formData.append("files", file),
                    );

                    const response = await fetch(
                        `/admin/api/media?content_type=product_variant&content_id=${contentId || 0}`,
                        {
                            method: "POST",
                            body: formData,
                        },
                    );

                    if (!response.ok) {
                        throw new Error("Upload failed");
                    }

                    const result = await response.json();

                    // Initialize media structure if not exists
                    if (!form.value.product.variants[variantIndex].media) {
                        form.value.product.variants[variantIndex].media = {
                            cover: [],
                            gallery: [],
                            icon: [],
                            video: [],
                            document: [],
                        };
                    }

                    // Add uploaded files to cover array
                    if (result.uploaded && result.uploaded.length > 0) {
                        result.uploaded.forEach((media) => {
                            form.value.product.variants[
                                variantIndex
                            ].media.cover.push({
                                id: media.id,
                                url: media.url,
                                file_name: media.file_name,
                                mime_type: media.mime_type,
                                title: "",
                                description: "",
                                content: "",
                                order_id:
                                    form.value.product.variants[variantIndex]
                                        .media.cover.length + 1,
                            });
                        });
                    }

                    // Clear input
                    event.target.value = "";
                } catch (error) {
                    console.error("Upload error:", error);
                    alert("Görsel yüklenirken bir hata oluştu");
                } finally {
                    uploadingVariantMedia.value = false;
                }
            };

            const removeVariantMedia = (variantIndex, mediaIndex) => {
                if (
                    confirm("Bu görseli kaldırmak istediğinizden emin misiniz?")
                ) {
                    form.value.product.variants[
                        variantIndex
                    ].media.cover.splice(mediaIndex, 1);
                }
            };

            // Lifecycle
            // TinyMCE Editor instances storage
            const tinyEditors = ref({});

            // Initialize TinyMCE Editor instances
            const initTinyEditors = async () => {
                try {
                    await nextTick();

                    // Check if supportedLanguages is loaded
                    if (
                        !supportedLanguages.value ||
                        typeof supportedLanguages.value !== "object" ||
                        Object.keys(supportedLanguages.value).length === 0
                    ) {
                        console.warn(
                            "supportedLanguages not loaded yet, skipping TinyMCE initialization",
                        );
                        return;
                    }

                    // Iterate over language codes
                    for (const langCode of Object.keys(
                        supportedLanguages.value,
                    )) {
                        // Description editor - NOT initialized by default, only when toggled
                        // (Skip description initialization here)

                        // Initialize full editor for body
                        const bodyId = `body_${langCode}`;
                        const bodyEl = document.getElementById(bodyId);

                        if (bodyEl && !tinyEditors.value[bodyId]) {
                            try {
                                await tinymce.init({
                                    target: bodyEl,
                                    height: 600,
                                    language: "tr_TR",
                                    language_url:
                                        "https://cdn.jsdelivr.net/npm/tinymce-lang/langs/tr_TR.js",
                                    menubar:
                                        "file edit view insert format tools table help",
                                    plugins: [
                                        "advlist",
                                        "autolink",
                                        "lists",
                                        "link",
                                        "image",
                                        "charmap",
                                        "preview",
                                        "anchor",
                                        "searchreplace",
                                        "visualblocks",
                                        "code",
                                        "fullscreen",
                                        "insertdatetime",
                                        "media",
                                        "table",
                                        "help",
                                        "wordcount",
                                        "emoticons",
                                        "codesample",
                                        "pagebreak",
                                        "nonbreaking",
                                        "directionality",
                                    ],
                                    toolbar:
                                        "undo redo | blocks fontfamily fontsize | bold italic underline strikethrough | " +
                                        "forecolor backcolor | alignleft aligncenter alignright alignjustify | " +
                                        "bullist numlist outdent indent | link image media table | " +
                                        "codesample code | fullscreen preview | removeformat help",
                                    toolbar_mode: "sliding",
                                    content_style:
                                        'body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; font-size: 15px; line-height: 1.6; }',
                                    font_family_formats:
                                        'System Font=-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; Andale Mono=andale mono,times; Arial=arial,helvetica,sans-serif; Arial Black=arial black,avant garde; Book Antiqua=book antiqua,palatino; Comic Sans MS=comic sans ms,sans-serif; Courier New=courier new,courier; Georgia=georgia,palatino; Helvetica=helvetica; Impact=impact,chicago; Symbol=symbol; Tahoma=tahoma,arial,helvetica,sans-serif; Terminal=terminal,monaco; Times New Roman=times new roman,times; Trebuchet MS=trebuchet ms,geneva; Verdana=verdana,geneva; Webdings=webdings; Wingdings=wingdings,zapf dingbats',
                                    font_size_formats:
                                        "8pt 10pt 12pt 14pt 15pt 16pt 18pt 24pt 36pt 48pt",
                                    block_formats:
                                        "Paragraph=p; Heading 1=h1; Heading 2=h2; Heading 3=h3; Heading 4=h4; Heading 5=h5; Heading 6=h6; Preformatted=pre; Code=code",
                                    image_title: true,
                                    automatic_uploads: true,
                                    file_picker_types: "image media",
                                    branding: false,
                                    promotion: false,
                                    image_class_list: [
                                        {
                                            title: "Responsive",
                                            value: "img-fluid",
                                        },
                                    ],
                                    codesample_languages: [
                                        { text: "Rust", value: "rust" },
                                        { text: "Go", value: "go" },
                                        { text: "Python", value: "python" },
                                        { text: "HTML/XML", value: "markup" },
                                        {
                                            text: "JavaScript",
                                            value: "javascript",
                                        },
                                        { text: "CSS", value: "css" },
                                        { text: "Ruby", value: "ruby" },
                                        { text: "Java", value: "java" },
                                        { text: "C", value: "c" },
                                        { text: "C#", value: "csharp" },
                                        { text: "C++", value: "cpp" },
                                        { text: "SQL", value: "sql" },
                                        { text: "Bash", value: "bash" },
                                        { text: "PHP", value: "php" },
                                    ],
                                    images_upload_handler: async (
                                        blobInfo,
                                        progress,
                                    ) => {
                                        const formData = new FormData();
                                        formData.append(
                                            "files",
                                            blobInfo.blob(),
                                            blobInfo.filename(),
                                        );

                                        const url = contentId
                                            ? `/admin/api/media?content_type=content&content_id=${contentId}`
                                            : `/admin/api/media?content_type=content`;

                                        try {
                                            const response = await fetch(url, {
                                                method: "POST",
                                                body: formData,
                                            });

                                            const data = await response.json();
                                            if (
                                                data.uploaded &&
                                                data.uploaded.length > 0
                                            ) {
                                                return data.uploaded[0].url;
                                            } else {
                                                throw new Error(
                                                    "No URL returned",
                                                );
                                            }
                                        } catch (error) {
                                            console.error(
                                                "Image upload failed:",
                                                error,
                                            );
                                            throw error;
                                        }
                                    },
                                    file_picker_callback: (
                                        callback,
                                        value,
                                        meta,
                                    ) => {
                                        // Video upload için file picker
                                        if (meta.filetype === "media") {
                                            const input =
                                                document.createElement("input");
                                            input.setAttribute("type", "file");
                                            input.setAttribute(
                                                "accept",
                                                "video/*",
                                            );

                                            input.onchange = async function () {
                                                const file = this.files[0];
                                                const formData = new FormData();
                                                formData.append("files", file);

                                                const url = contentId
                                                    ? `/admin/api/media?content_type=content&content_id=${contentId}`
                                                    : `/admin/api/media?content_type=content`;

                                                try {
                                                    const response =
                                                        await fetch(url, {
                                                            method: "POST",
                                                            body: formData,
                                                        });

                                                    const data =
                                                        await response.json();
                                                    if (
                                                        data.uploaded &&
                                                        data.uploaded.length > 0
                                                    ) {
                                                        callback(
                                                            data.uploaded[0]
                                                                .url,
                                                            {
                                                                title: file.name,
                                                                width: "100%",
                                                            },
                                                        );
                                                    }
                                                } catch (error) {
                                                    console.error(
                                                        "Video upload failed:",
                                                        error,
                                                    );
                                                    alert(
                                                        "Video yüklenirken hata oluştu",
                                                    );
                                                }
                                            };

                                            input.click();
                                        }
                                        // Poster image upload için
                                        else if (meta.filetype === "image") {
                                            const input =
                                                document.createElement("input");
                                            input.setAttribute("type", "file");
                                            input.setAttribute(
                                                "accept",
                                                "image/*",
                                            );

                                            input.onchange = async function () {
                                                const file = this.files[0];
                                                const formData = new FormData();
                                                formData.append("files", file);

                                                const url = contentId
                                                    ? `/admin/api/media?content_type=content&content_id=${contentId}`
                                                    : `/admin/api/media?content_type=content`;

                                                try {
                                                    const response =
                                                        await fetch(url, {
                                                            method: "POST",
                                                            body: formData,
                                                        });

                                                    const data =
                                                        await response.json();
                                                    if (
                                                        data.uploaded &&
                                                        data.uploaded.length > 0
                                                    ) {
                                                        callback(
                                                            data.uploaded[0]
                                                                .url,
                                                            {
                                                                alt: file.name,
                                                            },
                                                        );
                                                    }
                                                } catch (error) {
                                                    console.error(
                                                        "Poster upload failed:",
                                                        error,
                                                    );
                                                    alert(
                                                        "Poster görseli yüklenirken hata oluştu",
                                                    );
                                                }
                                            };

                                            input.click();
                                        }
                                    },
                                    setup: (editor) => {
                                        // Set default image attributes on insert
                                        editor.on("NodeChange", (e) => {
                                            if (
                                                e.element.nodeName === "IMG" &&
                                                !e.element.hasAttribute(
                                                    "data-mce-placeholder",
                                                )
                                            ) {
                                                if (!e.element.style.width) {
                                                    e.element.style.width =
                                                        "100%";
                                                    e.element.style.height =
                                                        "auto";
                                                    e.element.removeAttribute(
                                                        "width",
                                                    );
                                                    e.element.removeAttribute(
                                                        "height",
                                                    );
                                                }
                                            }
                                        });

                                        editor.on("change", () => {
                                            const content = editor.getContent();
                                            if (form.value.data[langCode]) {
                                                form.value.data[langCode].body =
                                                    content;
                                            }
                                        });

                                        editor.on("init", () => {
                                            if (
                                                form.value.data[langCode]?.body
                                            ) {
                                                editor.setContent(
                                                    form.value.data[langCode]
                                                        .body,
                                                );
                                            }
                                        });
                                    },
                                });

                                tinyEditors.value[bodyId] = true;
                            } catch (error) {
                                console.error(
                                    `Error initializing body editor for ${langCode}:`,
                                    error,
                                );
                            }
                        }
                    }
                } catch (error) {
                    console.error(
                        "Failed to initialize TinyMCE editors:",
                        error,
                    );
                }
            };

            // Cleanup TinyMCE instances
            const destroyTinyEditors = () => {
                tinymce.remove();
                tinyEditors.value = {};
            };

            // Kategori watch'ları kaldırıldı - attribute'lar artık kategori bağımsız

            // Content type değiştiğinde şablonu otomatik ayarla (sadece otomatik atanmış varsayılanlar değiştirilir)
            watch(
                () => form.value.content_type,
                (newType) => {
                    const newDefault =
                        defaultTemplates[newType] || defaultTemplates["page"];
                    const allDefaults = Object.values(defaultTemplates);
                    // Eğer mevcut şablon bilinen bir varsayılan şablon ise ve yeni varsayılanla farklıysa, güncelle
                    if (
                        allDefaults.includes(form.value.template) &&
                        form.value.template !== newDefault
                    ) {
                        form.value.template = newDefault;
                    }
                },
            );

            onMounted(async () => {
                await loadLanguages();
                await loadTemplates();

                // Edit modunda önce content'i yükle ki content_type doğru gelsin
                if (isEditing.value) {
                    await loadContent();
                    // Content yüklendikten sonra content_type'a göre term ve tag'leri yükle
                    await loadTerms();
                    await loadTags();

                    // Her iki liste de yüklendikten sonra term ve tag'leri ayır
                    if (form.value._allTermIds) {
                        separateTermsAndTags();
                    }

                    // Eğer product ise vocabulary'leri yükle
                    if (form.value.content_type === "product") {
                        await loadAvailableVocabularies();
                        // Mevcut ürün için seçili vocabulary'leri restore et
                        await restoreSelectedVocabularies();
                    }
                } else {
                    // Yeni oluşturma modunda URL'den gelen content_type'ı ayarla ve ilgili verileri yükle
                    if (contentType) {
                        form.value.content_type = contentType;
                    }

                    await loadTerms();
                    await loadTags();
                    form.value.product = {
                        currency: "TRY",
                        price: 0,
                        b2b_price: null,
                        old_price: null,
                        sku: "",
                        stock: 0,
                        on_sale: false,
                        attributes: {}, // YENİ: Taxonomy attributes
                        options: [],
                        variants: [],
                        // Pazaryeri entegrasyon alanları
                        barcode: "",
                        vat_rate: 20,
                        weight: null,
                        dimensional_weight: null,
                        dimensions: {
                            width: null,
                            height: null,
                            depth: null,
                        },
                        delivery_duration: 3,
                    };

                    // Eğer product ise vocabulary'leri yükle
                    if (form.value.content_type === "product") {
                        await loadAvailableVocabularies();
                    }
                }

                await loadParentPage();
                initMediaSortable();

                // Initialize TinyMCE Editor after everything else is loaded
                await initTinyEditors();
            });

            // Description editor toggle functionality
            const descriptionEditorStates = ref({}); // Track which editors are active

            const toggleDescriptionEditor = async (langCode) => {
                const descriptionId = `description_${langCode}`;
                const descriptionEl = document.getElementById(descriptionId);
                const toggleButton = document.getElementById(
                    `toggle_description_editor_${langCode}`,
                );
                const toggleText = document.getElementById(
                    `toggle_description_text_${langCode}`,
                );

                if (!descriptionEl || !toggleButton || !toggleText) {
                    console.error(
                        "Description editor elements not found for",
                        langCode,
                    );
                    return;
                }

                const isActive = descriptionEditorStates.value[langCode];

                if (!isActive) {
                    // Activate TinyMCE
                    try {
                        // First sync textarea value to form
                        if (descriptionEl.value && form.value.data[langCode]) {
                            form.value.data[langCode].description =
                                descriptionEl.value;
                        }

                        await tinymce.init({
                            target: descriptionEl,
                            height: 200,
                            language: "tr_TR",
                            language_url:
                                "https://cdn.jsdelivr.net/npm/tinymce-lang/langs/tr_TR.js",
                            menubar: "edit view insert format",
                            plugins:
                                "lists link code visualblocks wordcount charmap emoticons",
                            toolbar:
                                "undo redo | styles | bold italic underline | forecolor backcolor | alignleft aligncenter alignright | bullist numlist | link | code | removeformat",
                            content_style:
                                'body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; font-size: 15px; line-height: 1.6; }',
                            branding: false,
                            promotion: false,
                            setup: (editor) => {
                                editor.on("change", () => {
                                    const content = editor.getContent();
                                    if (form.value.data[langCode]) {
                                        form.value.data[langCode].description =
                                            content;
                                    }
                                });

                                editor.on("init", () => {
                                    if (
                                        form.value.data[langCode]?.description
                                    ) {
                                        editor.setContent(
                                            form.value.data[langCode]
                                                .description,
                                        );
                                    }
                                });
                            },
                        });

                        descriptionEditorStates.value[langCode] = true;
                        tinyEditors.value[descriptionId] = true;
                        toggleText.textContent = "Text Mode";
                        toggleButton.classList.remove("btn-outline-secondary");
                        toggleButton.classList.add("btn-outline-primary");
                    } catch (error) {
                        console.error(
                            `Error activating description editor for ${langCode}:`,
                            error,
                        );
                    }
                } else {
                    // Deactivate TinyMCE
                    try {
                        const editor = tinymce.get(descriptionId);
                        if (editor) {
                            // Sync content back to form before removing
                            const content = editor.getContent();
                            if (form.value.data[langCode]) {
                                form.value.data[langCode].description = content;
                            }

                            // Set textarea value
                            descriptionEl.value = content;

                            // Remove TinyMCE
                            editor.remove();
                        }

                        descriptionEditorStates.value[langCode] = false;
                        delete tinyEditors.value[descriptionId];
                        toggleText.textContent = "Rich Editor";
                        toggleButton.classList.remove("btn-outline-primary");
                        toggleButton.classList.add("btn-outline-secondary");
                    } catch (error) {
                        console.error(
                            `Error deactivating description editor for ${langCode}:`,
                            error,
                        );
                    }
                }
            };

            // Cleanup on unmount
            onBeforeUnmount(() => {
                destroyTinyEditors();
            });

            // Return for template
            return {
                loading,
                saving,
                isEditing,
                contentId,
                supportedLanguages,
                sortedLanguages,
                defaultLanguage,
                loadingLanguages,
                form,
                parentPage,
                pageBreadcrumbs,
                availableParentPages,
                loadingParents,
                parentSearch,
                availableTerms,
                loadingTerms,
                availableTags,
                selectedTags,
                loadingTags,
                tagSearch,
                tagSuggestions,
                selectedTagIndex,
                // Media Manager exports
                uploadingMedia,
                editingMedia,
                editingMediaLang,
                editingMediaIndex,
                editMediaFile,
                libraryMedia,
                libraryMeta,
                loadingLibrary,
                selectedLibraryMedia,
                currentLibraryLang,
                libraryFilters,
                groupedMedia: mediaGroupedMedia,
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
                availableTemplates,
                get_media_images_thumbnail,
                // Admin search bindings
                searchTerm,
                searchResults,
                searchLoading,
                debouncedSearch,
                performSearch,
                selectSearchResult,
                selectedSearchId,
                copyToClipboard,
                loadTemplates,

                // içerik metodları
                saveContent,
                loadParentPage,
                loadAvailableParents,
                searchParentPages,
                showParentModal,
                selectParent,
                getPageTitle,
                getPageSlug,
                removeParent,
                onContentTypeChange,
                toggleTerm,
                getTermTitle,
                onMasterCategoryChange,

                getTagTitle,
                searchTags,
                navigateTagSuggestions,
                addTag,
                addTagFromInput,
                removeTag,
                generateSlug,
                addSubContent,
                removeSubContent,

                // ürün metodları
                addProductOption,
                removeProductOption,
                clearProductOptions,
                addVariant,
                removeVariant,
                generateVariants,

                // varyant medya metodları
                editingVariant,
                uploadingVariantMedia,
                openVariantMediaModal,
                handleVariantMediaUpload,
                removeVariantMedia,

                // ürün özellikleri metodları
                availableVocabularies,
                selectedVocabularyIds,
                selectedVocabularies,
                loadingVocabularies,
                loadingAttributes,
                newAttributeValues,
                showAddNewAttributeValue,
                showAttributeSuggestions,
                attributeSuggestions,
                loadAvailableVocabularies,
                onVocabularySelectionChange,
                loadSelectedVocabularyAttributes,
                restoreSelectedVocabularies,
                onAttributeChange,
                addNewAttributeValue,
                getAttributeValueTitle,
                removeAttributeValue,
                cleanupInvalidAttributeValues,
                hasSelectedAttributes,
                selectedVariantVocabularyIds,
                selectedVocabulariesForVariants,
                showVariantGenerationModal,
                getSelectedAttributeCount,
                calculateVariantCount,
                generateVariantsFromSelectedAttributes,
                searchAttributeValues,
                selectAttributeSuggestion,
                hideAttributeSuggestions,

                // Description editor toggle
                toggleDescriptionEditor,
            };
        },
    });

    return app;
};

// test fonksiyonu - normalize display mantığını test et (konsol için)
window.testNormalizeDisplay = function () {
    const normalizeDisplay = (display) => {
        if (!display) return "";
        return display
            .split(" / ")
            .map((s) => s.trim())
            .sort()
            .join(" / ");
    };

    const test1 = "Mavi / 1KG / Demir";
    const test2 = "Demir / Mavi / 1KG";
    const test3 = "1KG / Demir / Mavi";

    console.log("=== Normalize Edilmiş Text ===");
    console.log(`"${test1}" → "${normalizeDisplay(test1)}"`);
    console.log(`"${test2}" → "${normalizeDisplay(test2)}"`);
    console.log(`"${test3}" → "${normalizeDisplay(test3)}"`);
    console.log(
        "Tüm Eşleşmeler",
        normalizeDisplay(test1) === normalizeDisplay(test2) &&
            normalizeDisplay(test2) === normalizeDisplay(test3),
    );
    console.log("===============================");
};
