/**
 * Global Add to Cart Modal System
 *
 * Bu modül, projenin her yerinde kullanılabilecek bir sepete ekleme modal sistemi sağlar.
 *
 * Kullanım:
 *   - window.openAddToCartModal(productId)  → Modal açarak ürünü sepete ekleme
 *   - window.executeAddToCart(productId, variantKey, quantity) → Direkt sepete ekleme (modal olmadan)
 *   - window.cartGetThumbnail(path, size, crop) → Thumbnail URL oluşturma
 */
(function () {
    "use strict";

    const { createApp, ref, computed } = Vue;

    // ─── Thumbnail Helper ───────────────────────────────────────────────
    const getThumbnail = (path, size = "800x800", crop = "center") => {
        if (!path) return "/static/no_image.png";
        let cleanPath = path;
        if (path.startsWith("/media/uploads/")) {
            cleanPath = path.substring(15);
        } else if (path.startsWith("media/uploads/")) {
            cleanPath = path.substring(14);
        } else if (path.startsWith("/media/")) {
            cleanPath = path.substring(7);
        }
        return `/media/thumb/${size}/${crop}/${cleanPath}`;
    };

    // Expose globally
    window.cartGetThumbnail = getThumbnail;

    // ─── Bootstrap Modal Reference ─────────────────────────────────────
    let bsModal = null;

    // ─── Direct Add to Cart API Call (no modal) ─────────────────────────
    const executeAddToCart = async (productId, variantKey, qty = 1) => {
        const cartData = {
            product_id: productId,
            variant_key: variantKey,
            quantity: qty,
        };

        try {
            const response = await fetch("/api/cart/items", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(cartData),
            });
            const result = await response.json();

            if (result.success) {
                console.log("Product added to cart:", result);
                // Update navbar cart badge
                const badgeEl = document.getElementById("basket_item_count");
                if (badgeEl) {
                    badgeEl.textContent = result.data.item_count;
                }

                // Close modal if open
                if (bsModal) {
                    try {
                        bsModal.hide();
                    } catch (e) {
                        /* ignore */
                    }
                }

                const swalResult = await Swal.fire({
                    icon: "success",
                    title: "Sepete Eklendi!",
                    html: `
                        <p class="mb-2"><strong>${result.data.product_title}</strong></p>
                        ${result.data.variant_display ? `<p class="text-muted small mb-2">${result.data.variant_display}</p>` : ""}
                        <p class="mb-0">Miktar: ${result.data.quantity}</p>
                    `,
                    showCancelButton: true,
                    confirmButtonText: "Sepete Git",
                    cancelButtonText: "Alışverişe Devam",
                    confirmButtonColor: "#0d6efd",
                    cancelButtonColor: "#6c757d",
                });

                if (swalResult.isConfirmed) {
                    window.location.href = "/my-cart";
                }

                return { success: true, data: result.data };
            } else {
                Swal.fire({
                    icon: "error",
                    title: "Hata!",
                    text:
                        result.error ||
                        "Ürün sepete eklenirken bir hata oluştu.",
                    confirmButtonText: "Tamam",
                    confirmButtonColor: "#dc3545",
                });
                return { success: false, error: result.error };
            }
        } catch (error) {
            console.error("Sepete ekleme hatası:", error);
            Swal.fire({
                icon: "error",
                title: "Bağlantı Hatası",
                text: "Sunucuya bağlanırken bir hata oluştu. Lütfen tekrar deneyin.",
                confirmButtonText: "Tamam",
                confirmButtonColor: "#dc3545",
            });
            return { success: false, error: error.message };
        }
    };

    // Expose globally
    window.executeAddToCart = executeAddToCart;

    // ─── Init Vue App on DOMContentLoaded ───────────────────────────────
    document.addEventListener("DOMContentLoaded", () => {
        const modalEl = document.getElementById("globalCartModal");
        if (!modalEl) {
            console.warn(
                "Add to Cart Modal: #globalCartModal element not found in DOM.",
            );
            return;
        }

        const currentLanguage = modalEl.dataset.language || "tr";

        // Initialize Bootstrap Modal
        bsModal = new bootstrap.Modal(modalEl);

        // ─── Vue App ────────────────────────────────────────────────────
        const app = createApp({
            delimiters: ["[[", "]]"],
            setup() {
                // State
                const selectedProduct = ref(null);
                const productVariants = ref([]);
                const modalSelectedOptions = ref({});
                const currentVariant = ref(null);
                const quantity = ref(1);
                const maxQuantity = ref(99);
                const isLoading = ref(false);

                // ── Variant Helpers ─────────────────────────────────────

                const getUniqueOptionsFromVariants = (variants) => {
                    const optionsMap = new Map();
                    variants.forEach((variant) => {
                        if (
                            variant.option_values &&
                            typeof variant.option_values === "object"
                        ) {
                            Object.entries(variant.option_values).forEach(
                                ([name, value]) => {
                                    if (!optionsMap.has(name))
                                        optionsMap.set(name, new Set());
                                    optionsMap.get(name).add(value);
                                },
                            );
                        }
                    });
                    return Array.from(optionsMap.entries()).map(
                        ([name, valuesSet]) => ({
                            name,
                            values: Array.from(valuesSet).join(", "),
                        }),
                    );
                };

                const uniqueOptions = computed(() =>
                    getUniqueOptionsFromVariants(productVariants.value),
                );

                const getOptionValues = (valuesString) => {
                    return valuesString
                        ? valuesString.split(",").map((v) => v.trim())
                        : [];
                };

                const isOptionAvailable = (optionName, value) => {
                    return productVariants.value.some((variant) => {
                        if (!variant.option_values) return false;
                        if (
                            variant.option_values[optionName]?.trim() !==
                            value.trim()
                        )
                            return false;

                        return uniqueOptions.value.every((option) => {
                            if (option.name === optionName) return true;
                            const sel = modalSelectedOptions.value[option.name];
                            if (!sel) return true;
                            return (
                                variant.option_values[option.name]?.trim() ===
                                sel.trim()
                            );
                        });
                    });
                };

                const selectOption = (optionName, value) => {
                    modalSelectedOptions.value[optionName] = value;
                    updateCurrentVariant();
                };

                const updateCurrentVariant = () => {
                    const allSelected = uniqueOptions.value.every(
                        (o) => modalSelectedOptions.value[o.name],
                    );
                    if (!allSelected) {
                        currentVariant.value = null;
                        return;
                    }

                    currentVariant.value =
                        productVariants.value.find((variant) => {
                            return uniqueOptions.value.every((option) => {
                                const sel =
                                    modalSelectedOptions.value[option.name];
                                return (
                                    variant.option_values?.[
                                        option.name
                                    ]?.trim() === sel.trim()
                                );
                            });
                        }) || null;

                    // Update max quantity based on selected variant stock
                    if (currentVariant.value) {
                        maxQuantity.value = currentVariant.value.stock || 99;
                        if (quantity.value > maxQuantity.value) {
                            quantity.value = Math.max(1, maxQuantity.value);
                        }
                    }
                };

                const getButtonClass = (optionName, value) => {
                    const isSelected =
                        modalSelectedOptions.value[optionName] === value;
                    if (isSelected) return "btn-primary";
                    return "btn-outline-primary";
                };

                // ── Quantity Controls ───────────────────────────────────

                const increaseQuantity = () => {
                    if (quantity.value < maxQuantity.value) quantity.value++;
                };

                const decreaseQuantity = () => {
                    if (quantity.value > 1) quantity.value--;
                };

                const validateQuantity = () => {
                    if (quantity.value < 1) quantity.value = 1;
                    if (quantity.value > maxQuantity.value)
                        quantity.value = maxQuantity.value;
                };

                // ── Computed Properties ─────────────────────────────────

                const modalProductImage = computed(() => {
                    // Variant cover image
                    if (currentVariant.value?.media?.cover?.[0]?.url) {
                        return getThumbnail(
                            currentVariant.value.media.cover[0].url,
                            "200x200",
                        );
                    }

                    // Product cover image from lang data
                    const langs = selectedProduct.value?.data?.langs;
                    if (
                        langs &&
                        langs[currentLanguage]?.media?.cover?.[0]?.url
                    ) {
                        return getThumbnail(
                            langs[currentLanguage].media.cover[0].url,
                            "200x200",
                        );
                    }

                    return "/static/no_image.png";
                });

                const formattedModalPrice = computed(() => {
                    if (currentVariant.value)
                        return (
                            currentVariant.value.display_price_formatted || ""
                        );
                    return selectedProduct.value?.price_formatted || "";
                });

                const stockText = computed(() => {
                    // Products with variants
                    if (productVariants.value.length > 0) {
                        if (!currentVariant.value)
                            return "Lütfen seçenekleri belirleyin";
                        const s = currentVariant.value.stock;
                        return s > 10
                            ? "Stokta var"
                            : s > 0
                              ? `Son ${s} adet`
                              : "Tükendi";
                    }
                    // Products without variants
                    const stock = selectedProduct.value?.product?.stock || 0;
                    return stock > 10
                        ? "Stokta var"
                        : stock > 0
                          ? `Son ${stock} adet`
                          : "Tükendi";
                });

                const addToCartDisabled = computed(() => {
                    if (isLoading.value) return true;
                    if (
                        productVariants.value.length > 0 &&
                        !currentVariant.value
                    )
                        return true;
                    if (currentVariant.value && currentVariant.value.stock <= 0)
                        return true;
                    if (productVariants.value.length === 0) {
                        const stock =
                            selectedProduct.value?.product?.stock || 0;
                        if (stock <= 0) return true;
                    }
                    if (
                        quantity.value < 1 ||
                        quantity.value > maxQuantity.value
                    )
                        return true;
                    return false;
                });

                // ── Actions ─────────────────────────────────────────────

                const addToCart = async () => {
                    if (
                        productVariants.value.length > 0 &&
                        !currentVariant.value
                    ) {
                        Swal.fire({
                            icon: "warning",
                            title: "Seçim Yapın",
                            text: "Lütfen seçenekleri belirleyin.",
                            confirmButtonText: "Tamam",
                            confirmButtonColor: "#0d6efd",
                        });
                        return;
                    }

                    isLoading.value = true;
                    try {
                        await executeAddToCart(
                            selectedProduct.value.id,
                            currentVariant.value?.option_values_display || null,
                            quantity.value,
                        );
                    } finally {
                        isLoading.value = false;
                    }
                };

                /**
                 * Open the modal for a given product ID.
                 * Fetches product data from API, initializes variant state, shows modal.
                 */
                const openModal = async (productId) => {
                    isLoading.value = true;

                    try {
                        const response = await fetch(
                            `/api/products/${productId}`,
                        );
                        const result = await response.json();

                        if (result.success && result.results) {
                            const productData = result.results;
                            const variants =
                                productData.product?.variants || [];

                            // Set state
                            selectedProduct.value = productData;
                            productVariants.value = variants;
                            quantity.value = 1;

                            // Set max quantity based on stock
                            if (variants.length === 0) {
                                maxQuantity.value =
                                    productData.product?.stock || 99;
                            } else {
                                maxQuantity.value = 99;
                            }

                            // Reset variant selection
                            const opts = {};
                            const unique =
                                getUniqueOptionsFromVariants(variants);
                            unique.forEach((o) => (opts[o.name] = null));
                            modalSelectedOptions.value = opts;
                            currentVariant.value = null;

                            isLoading.value = false;
                            bsModal.show();
                        } else {
                            isLoading.value = false;
                            Swal.fire({
                                icon: "error",
                                title: "Hata",
                                text: "Ürün bilgileri yüklenemedi.",
                                confirmButtonText: "Tamam",
                                confirmButtonColor: "#dc3545",
                            });
                        }
                    } catch (error) {
                        isLoading.value = false;
                        console.error("Error fetching product details:", error);
                        Swal.fire({
                            icon: "error",
                            title: "Bağlantı Hatası",
                            text: "Ürün bilgileri yüklenirken bir hata oluştu.",
                            confirmButtonText: "Tamam",
                            confirmButtonColor: "#dc3545",
                        });
                    }
                };

                /**
                 * Open the modal with pre-loaded product data (no API call needed).
                 * Useful for product_detail page where data is already available.
                 *
                 * @param {Object} params
                 * @param {Object} params.product - Product data object (must have .id)
                 * @param {Array}  params.variants - Product variants array
                 * @param {number} [params.stock] - Product stock (for non-variant products)
                 */
                const openModalWithData = (params) => {
                    const { product, variants = [], stock = 99 } = params;

                    selectedProduct.value = product;
                    productVariants.value = variants;
                    quantity.value = 1;

                    if (variants.length === 0) {
                        maxQuantity.value = stock;
                    } else {
                        maxQuantity.value = 99;
                    }

                    // Reset variant selection
                    const opts = {};
                    const unique = getUniqueOptionsFromVariants(variants);
                    unique.forEach((o) => (opts[o.name] = null));
                    modalSelectedOptions.value = opts;
                    currentVariant.value = null;

                    bsModal.show();
                };

                // ── Expose Global Functions ─────────────────────────────
                window.openAddToCartModal = openModal;
                window.openAddToCartModalWithData = openModalWithData;

                return {
                    selectedProduct,
                    productVariants,
                    modalSelectedOptions,
                    currentVariant,
                    quantity,
                    maxQuantity,
                    isLoading,
                    uniqueOptions,
                    getOptionValues,
                    isOptionAvailable,
                    selectOption,
                    getButtonClass,
                    increaseQuantity,
                    decreaseQuantity,
                    validateQuantity,
                    modalProductImage,
                    formattedModalPrice,
                    stockText,
                    addToCartDisabled,
                    addToCart,
                };
            },
        });

        app.mount("#globalCartModal");
    });
})();
