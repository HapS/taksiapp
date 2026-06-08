(function () {
    "use strict";

    const { createApp, ref, computed, onMounted } = Vue;

    const initReviewsApp = () => {
        const appEl = document.getElementById("reviewsApp");
        if (!appEl) return;

        const productId = appEl.dataset.productId;
        const currentLang = appEl.dataset.lang;
        const userIdRaw = appEl.dataset.userId;
        const userId = userIdRaw ? Number(userIdRaw) : null;
        const isAuthenticated = appEl.dataset.authenticated === "true";

        if (!productId) {
            console.error("Reviews app: productId not found");
            return;
        }

        const app = createApp({
            delimiters: ["[[", "]]"],
            setup() {
                const comments = ref([]);
                const pagination = ref({
                    total: 0,
                    page: 1,
                    per_page: 20,
                    total_pages: 1,
                });
                const stats = ref({
                    total: 0,
                    average_star: null,
                });
                const commentForm = ref({ content: "" });
                const commentRating = ref(5);
                const isSubmitting = ref(false);
                const isLoadingComments = ref(true);
                const formError = ref("");
                const formSuccess = ref("");
                const deletingId = ref(null);

                const averageRating = computed(() => {
                    if (stats.value.average_star === null || stats.value.average_star === undefined) return 0;
                    return Math.round(stats.value.average_star * 10) / 10;
                });

                const visiblePages = computed(() => {
                    const total = pagination.value.total_pages;
                    const current = pagination.value.page;
                    const pages = [];
                    let start = Math.max(1, current - 2);
                    let end = Math.min(total, current + 2);
                    if (end - start < 4) {
                        if (start === 1) end = Math.min(total, start + 4);
                        else start = Math.max(1, end - 4);
                    }
                    for (let i = start; i <= end; i++) pages.push(i);
                    return pages;
                });

                const fetchComments = async (page = 1) => {
                    isLoadingComments.value = true;
                    try {
                        const params = new URLSearchParams({
                            content_type: "product",
                            content_id: productId,
                            page: page.toString(),
                            per_page: "20",
                            published_only: "true",
                        });
                        const response = await fetch(`/api/comments?${params}`);
                        const data = await response.json();
                        if (data.status === "success") {
                            comments.value = data.data;
                            pagination.value = data.pagination;
                            if (data.stats) {
                                stats.value = data.stats;
                            }
                        }
                    } catch (error) {
                        console.error("Yorumlar yüklenirken hata:", error);
                    } finally {
                        isLoadingComments.value = false;
                    }
                };

                const submitComment = async () => {
                    if (!commentForm.value.content.trim()) return;
                    isSubmitting.value = true;
                    formError.value = "";
                    formSuccess.value = "";
                    try {
                        const response = await fetch("/api/comments", {
                            method: "POST",
                            headers: { "Content-Type": "application/json" },
                            body: JSON.stringify({
                                content_type: "product",
                                content_id: parseInt(productId),
                                content: commentForm.value.content,
                                star: commentRating.value,
                            }),
                        });
                        const data = await response.json();
                        if (data.status === "success") {
                            formSuccess.value =
                                "Yorumunuz başarıyla gönderildi!";
                            commentForm.value.content = "";
                            commentRating.value = 5;
                            await fetchComments(pagination.value.page);
                        } else {
                            formError.value =
                                data.message ||
                                "Yorum gönderilirken bir hata oluştu.";
                        }
                    } catch (error) {
                        formError.value =
                            "Yorum gönderilirken bir hata oluştu.";
                        console.error("Yorum gönderme hatası:", error);
                    } finally {
                        isSubmitting.value = false;
                    }
                };

                const deleteComment = async (commentId) => {
                    const result = await Swal.fire({
                        icon: "warning",
                        title: "Yorum Sil",
                        text: "Bu yorumu silmek istediğinize emin misiniz?",
                        showCancelButton: true,
                        confirmButtonText: "Sil",
                        cancelButtonText: "İptal",
                        confirmButtonColor: "#dc3545",
                        cancelButtonColor: "#6c757d",
                    });

                    if (!result.isConfirmed) return;

                    deletingId.value = commentId;
                    try {
                        const response = await fetch(
                            `/api/comments/${commentId}`,
                            {
                                method: "DELETE",
                            },
                        );
                        const data = await response.json();
                        if (data.status === "success") {
                            await fetchComments(pagination.value.page);
                            Swal.fire({
                                icon: "success",
                                title: "Silindi",
                                text: "Yorum başarıyla silindi.",
                                toast: true,
                                position: "top-end",
                                showConfirmButton: false,
                                timer: 2000,
                            });
                        } else {
                            Swal.fire({
                                icon: "error",
                                title: "Hata",
                                text:
                                    data.message ||
                                    "Yorum silinirken bir hata oluştu.",
                                confirmButtonText: "Tamam",
                                confirmButtonColor: "#dc3545",
                            });
                        }
                    } catch (error) {
                        Swal.fire({
                            icon: "error",
                            title: "Hata",
                            text: "Yorum silinirken bir hata oluştu.",
                            confirmButtonText: "Tamam",
                            confirmButtonColor: "#dc3545",
                        });
                        console.error("Yorum silme hatası:", error);
                    } finally {
                        deletingId.value = null;
                    }
                };

                const changePage = (page) => {
                    if (page >= 1 && page <= pagination.value.total_pages) {
                        fetchComments(page);
                    }
                };

                const formatDate = (dateString) => {
                    if (!dateString) return "";
                    const date = new Date(dateString);
                    const now = new Date();
                    const diffDays = Math.floor(
                        Math.abs(now - date) / (1000 * 60 * 60 * 24),
                    );
                    if (diffDays === 0) return "Bugün";
                    if (diffDays === 1) return "Dün";
                    if (diffDays < 7) return `${diffDays} gün önce`;
                    if (diffDays < 30)
                        return `${Math.floor(diffDays / 7)} hafta önce`;
                    if (diffDays < 365)
                        return `${Math.floor(diffDays / 30)} ay önce`;
                    return `${Math.floor(diffDays / 365)} yıl önce`;
                };

                const isOwnComment = (commentUserId) => {
                    return userId && commentUserId === userId;
                };

                onMounted(() => {
                    fetchComments();
                });

                return {
                    comments,
                    pagination,
                    stats,
                    commentForm,
                    commentRating,
                    isSubmitting,
                    isLoadingComments,
                    formError,
                    formSuccess,
                    currentLang,
                    userId,
                    isAuthenticated,
                    deletingId,
                    averageRating,
                    visiblePages,
                    isOwnComment,
                    fetchComments,
                    submitComment,
                    deleteComment,
                    changePage,
                    formatDate,
                };
            },
        });

        app.mount("#reviewsApp");
    };

    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", initReviewsApp);
    } else {
        initReviewsApp();
    }
})();
