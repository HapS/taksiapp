/**
 * Form Builder Component for Content Editor
 * Handles dynamic form creation with drag and drop
 */

const FormBuilderComponent = {
    props: ['modelValue'],
    emits: ['update:modelValue'],
    template: `
        <div class="form-builder">
            <div class="row">
                <!-- Sidebar: Components -->
                <div class="col-md-3">
                    <div class="card shadow-sm mb-4">
                        <div class="card-header bg-light">
                            <h6 class="mb-0"><i class="bi bi-plus-square me-2"></i>Alan Ekle</h6>
                        </div>
                        <div class="card-body p-2">
                            <div class="list-group list-group-flush">
                                <button type="button" class="list-group-item list-group-item-action border-dashed mb-1 rounded" 
                                        draggable="true" @dragstart="onDragStart($event, 'text')">
                                    <i class="bi bi-type me-2"></i> Metin Kutusu
                                </button>
                                <button type="button" class="list-group-item list-group-item-action border-dashed mb-1 rounded" 
                                        draggable="true" @dragstart="onDragStart($event, 'textarea')">
                                    <i class="bi bi-textarea-t me-2"></i> Metin Alanı
                                </button>
                                <button type="button" class="list-group-item list-group-item-action border-dashed mb-1 rounded" 
                                        draggable="true" @dragstart="onDragStart($event, 'number')">
                                    <i class="bi bi-123 me-2"></i> Sayı Alanı
                                </button>
                                <button type="button" class="list-group-item list-group-item-action border-dashed mb-1 rounded" 
                                        draggable="true" @dragstart="onDragStart($event, 'email')">
                                    <i class="bi bi-envelope me-2"></i> E-posta
                                </button>
                                <button type="button" class="list-group-item list-group-item-action border-dashed mb-1 rounded" 
                                        draggable="true" @dragstart="onDragStart($event, 'select')">
                                    <i class="bi bi-list-ul me-2"></i> Seçim Kutusu
                                </button>
                                <button type="button" class="list-group-item list-group-item-action border-dashed mb-1 rounded" 
                                        draggable="true" @dragstart="onDragStart($event, 'radio')">
                                    <i class="bi bi-circle me-2"></i> Radyo Butonu
                                </button>
                                <button type="button" class="list-group-item list-group-item-action border-dashed mb-1 rounded" 
                                        draggable="true" @dragstart="onDragStart($event, 'checkbox')">
                                    <i class="bi bi-check-square me-2"></i> Onay Kutusu
                                </button>
                                <button type="button" class="list-group-item list-group-item-action border-dashed mb-1 rounded" 
                                        draggable="true" @dragstart="onDragStart($event, 'date')">
                                    <i class="bi bi-calendar me-2"></i> Tarih Seçici
                                </button>
                                <button type="button" class="list-group-item list-group-item-action border-dashed mb-1 rounded" 
                                        draggable="true" @dragstart="onDragStart($event, 'file')">
                                    <i class="bi bi-upload me-2"></i> Dosya Yükleme
                                </button>
                            </div>
                        </div>
                    </div>

                    <!-- Toolbar -->
                    <div class="d-grid gap-2">
                        <button type="button" class="btn btn-outline-primary btn-sm" @click="showPreview = true" :disabled="fields.length === 0">
                            <i class="bi bi-eye me-1"></i> Önizle
                        </button>
                        <button type="button" class="btn btn-outline-danger btn-sm" @click="clearForm" :disabled="fields.length === 0">
                            <i class="bi bi-trash me-1"></i> Formu Temizle
                        </button>
                    </div>
                </div>

                <!-- Canvas: Form Designer -->
                <div class="col-md-6">
                    <div class="card shadow-sm">
                        <div class="card-header bg-white d-flex justify-content-between align-items-center">
                            <h6 class="mb-0">Form Tasarımı</h6>
                            <span class="badge bg-secondary text-white">[[ fields.length ]] Alan</span>
                        </div>
                        <div class="card-body bg-light p-3" 
                             @dragover.prevent 
                             @drop="onDrop"
                             ref="canvasRef"
                             style="min-height: 400px; border: 2px dashed #ddd; border-radius: 0px;">
                            
                            <div v-if="fields.length === 0" class="text-center text-muted py-5">
                                <i class="bi bi-mouse display-4 d-block mb-3"></i>
                                <p>Elemanları buraya sürükleyin veya sol taraftan seçin</p>
                            </div>

                            <div v-for="(field, index) in fields" 
                                 :key="field.id" 
                                 class="card mb-2 form-item-card"
                                 :class="{ 'border-primary shadow-sm': selectedFieldId === field.id }"
                                 @click="selectField(field)">
                                <div class="card-body p-3">
                                    <div class="d-flex justify-content-between align-items-start mb-2">
                                        <div>
                                            <span class="badge bg-info text-white me-2">[[ getTypeLabel(field.type) ]]</span>
                                            <strong v-if="field.label">[[ field.label ]]</strong>
                                            <span v-else class="text-muted italic small">Etiketsiz Alan</span>
                                            <span v-if="field.required" class="text-danger ms-1">*</span>
                                            <span class="badge bg-secondary ms-2 x-small">[[ field.col ]]</span>
                                        </div>
                                        <div class="btn-group btn-group-sm">
                                            <button type="button" class="btn btn-sm btn-outline-danger border-0" @click.stop="removeField(index)">
                                                <i class="bi bi-x-lg"></i>
                                            </button>
                                        </div>
                                    </div>
                                    
                                    <!-- Simple Preview of the field -->
                                    <div class="preview-mock small text-muted border rounded p-2 bg-white">
                                        <div v-if="['text', 'email', 'number', 'date'].includes(field.type)">
                                            <input type="text" class="form-control form-control-sm" disabled :placeholder="field.placeholder">
                                        </div>
                                        <div v-else-if="field.type === 'textarea'">
                                            <textarea class="form-control form-control-sm" disabled rows="2" :placeholder="field.placeholder"></textarea>
                                        </div>
                                        <div v-else-if="field.type === 'select'">
                                            <select class="form-control form-control-sm" disabled>
                                                <option>[[ field.options[0] || 'Seçenek...' ]]</option>
                                            </select>
                                        </div>
                                        <div v-else-if="field.type === 'radio'">
                                            <div class="form-check">
                                                <input class="form-check-input" type="radio" disabled checked>
                                                <label class="form-check-label">[[ field.options[0] || 'Seçenek' ]]</label>
                                            </div>
                                        </div>
                                        <div v-else-if="field.type === 'checkbox'">
                                            <div class="form-check">
                                                <input class="form-check-input" type="checkbox" disabled checked>
                                                <label class="form-check-label">[[ field.label ]]</label>
                                            </div>
                                        </div>
                                        <div v-else-if="field.type === 'file'">
                                            <i class="bi bi-upload me-1"></i> Dosya seç...
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Properties Panel -->
                <div class="col-md-3">
                    <div class="card shadow-sm sticky-top" style="top: 20px;">
                        <div class="card-header bg-light">
                            <h6 class="mb-0"><i class="bi bi-sliders me-2"></i>Özellikler</h6>
                        </div>
                        <div class="card-body p-3 overflow-auto" style="max-height: calc(100vh - 200px);">
                            <div v-if="selectedField">
                                <div class="mb-3">
                                    <label class="form-label small fw-bold">Genişlik (Grid)</label>
                                    <select class="form-select form-select-sm" v-model="selectedField.col">
                                        <option value="col-12">Tam Genişlik (%100)</option>
                                        <option value="col-md-6">Yarım (%50)</option>
                                        <option value="col-md-4">Üçte Bir (%33)</option>
                                        <option value="col-md-3">Dörtte Bir (%25)</option>
                                    </select>
                                </div>
                                <div class="mb-3">
                                    <label class="form-label small fw-bold">Alan Etiketi</label>
                                    <input type="text" class="form-control form-control-sm" v-model="selectedField.label">
                                </div>
                                <div class="mb-3">
                                    <label class="form-label small fw-bold">Alan Adı (name)</label>
                                    <input type="text" class="form-control form-control-sm" v-model="selectedField.name">
                                    <div class="form-text x-small">Veritabanında saklanacak anahtar.</div>
                                </div>
                                <div class="mb-3" v-if="!['checkbox', 'date', 'file', 'radio', 'select'].includes(selectedField.type)">
                                    <label class="form-label small fw-bold">Placeholder</label>
                                    <input type="text" class="form-control form-control-sm" v-model="selectedField.placeholder">
                                </div>
                                <div class="mb-3 form-check">
                                    <input type="checkbox" class="form-check-input" v-model="selectedField.required" id="fieldReq">
                                    <label class="form-check-label small" for="fieldReq">Zorunlu Alan</label>
                                </div>

                                <!-- Options Editor for Select/Radio -->
                                <div v-if="['select', 'radio'].includes(selectedField.type)" class="mt-4 pt-3 border-top">
                                    <label class="form-label small fw-bold d-flex justify-content-between">
                                        Seçenekler
                                        <button type="button" class="btn btn-link btn-sm p-0" @click="addOption">Ekle</button>
                                    </label>
                                    <div class="options-container">
                                        <div v-for="(opt, oIdx) in selectedField.options" :key="oIdx" class="input-group input-group-sm mb-2">
                                            <input type="text" class="form-control" v-model="selectedField.options[oIdx]">
                                            <button class="btn btn-outline-danger" type="button" @click="removeOption(oIdx)">
                                                <i class="bi bi-trash"></i>
                                            </button>
                                        </div>
                                    </div>
                                </div>

                                <!-- Visibility Rules -->
                                <div class="mt-4 pt-3 border-top">
                                    <label class="form-label small fw-bold d-flex justify-content-between">
                                        Koşullu Görünürlük
                                        <button type="button" class="btn btn-link btn-sm p-0" @click="addRule">Ekle</button>
                                    </label>
                                    <div v-if="selectedField.visibilityRules && selectedField.visibilityRules.length > 0">
                                        <div v-for="(rule, rIdx) in selectedField.visibilityRules" :key="rIdx" class="p-2 border rounded bg-light mb-2 position-relative">
                                            <button type="button" class="btn-close x-small position-absolute top-0 end-0 m-1" @click="removeRule(rIdx)"></button>
                                            
                                            <div class="mb-2">
                                                <label class="x-small fw-bold">Bağlı Alan:</label>
                                                <select class="form-select form-select-sm" v-model="rule.targetField">
                                                    <option v-for="f in getPotentialTargetFields()" :key="f.id" :value="f.id">
                                                        [[ f.label || f.name ]]
                                                    </option>
                                                </select>
                                            </div>
                                            
                                            <div>
                                                <label class="x-small fw-bold">Eğer Değeri:</label>
                                                <div v-if="getRuleFieldOptions(rule.targetField).length > 0" class="rule-values-list mt-1">
                                                    <div v-for="opt in getRuleFieldOptions(rule.targetField)" :key="opt" class="form-check x-small">
                                                        <input class="form-check-input" type="checkbox" :value="opt" v-model="rule.values" :id="'rule-'+rIdx+'-'+opt">
                                                        <label class="form-check-label" :for="'rule-'+rIdx+'-'+opt">[[ opt ]]</label>
                                                    </div>
                                                </div>
                                                <input v-else type="text" class="form-control form-control-sm" v-model="rule.value" placeholder="Karşılaştırılacak değer">
                                            </div>
                                        </div>
                                    </div>
                                    <div v-else class="text-muted x-small">Hiç kural eklenmemiş.</div>
                                </div>
                            </div>
                            <div v-else class="text-center text-muted py-4">
                                <p class="small">Düzenlemek için bir alan seçin</p>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Preview Modal -->
            <teleport to="body">
                <div v-if="showPreview" class="modal fade show d-block" tabindex="-1" style="background: rgba(0,0,0,0.5); z-index: 9999;">
                    <div class="modal-dialog modal-xl">
                        <div class="modal-content">
                            <div class="modal-header">
                                <h5 class="modal-title">Form Önizleme</h5>
                                <button type="button" class="btn-close" @click="showPreview = false"></button>
                            </div>
                            <div class="modal-body bg-light">
                                <div class="card shadow-sm mx-auto" style="max-width: 900px;">
                                    <div class="card-body">
                                        <form @submit.prevent="previewSubmit">
                                            <div class="row">
                                                <div v-for="f in fields" :key="f.id" :class="f.col || 'col-12'" class="mb-3" v-show="isFieldVisible(f)">
                                                    <label class="form-label fw-bold small">
                                                        [[ f.label ]]
                                                        <span v-if="f.required" class="text-danger">*</span>
                                                    </label>
                                                    
                                                    <template v-if="['text', 'email', 'number', 'date'].includes(f.type)">
                                                        <input :type="f.type" class="form-control" :placeholder="f.placeholder" v-model="previewData[f.id]">
                                                    </template>
                                                    
                                                    <template v-else-if="f.type === 'textarea'">
                                                        <textarea class="form-control" :placeholder="f.placeholder" rows="3" v-model="previewData[f.id]"></textarea>
                                                    </template>
                                                    
                                                    <template v-else-if="f.type === 'select'">
                                                        <select class="form-select" v-model="previewData[f.id]">
                                                            <option value="">Seçiniz...</option>
                                                            <option v-for="opt in f.options" :key="opt" :value="opt">[[ opt ]]</option>
                                                        </select>
                                                    </template>
                                                    
                                                    <template v-else-if="f.type === 'radio'">
                                                        <div v-for="opt in f.options" :key="opt" class="form-check">
                                                            <input class="form-check-input" type="radio" :name="'prev-'+f.id" :value="opt" v-model="previewData[f.id]">
                                                            <label class="form-check-label">[[ opt ]]</label>
                                                        </div>
                                                    </template>
                                                    
                                                    <template v-else-if="f.type === 'checkbox'">
                                                        <div class="form-check">
                                                            <input class="form-check-input" type="checkbox" v-model="previewData[f.id]">
                                                            <label class="form-check-label">Evet / Onaylıyorum</label>
                                                        </div>
                                                    </template>
                                                    
                                                    <template v-else-if="f.type === 'file'">
                                                        <input type="file" class="form-control">
                                                    </template>
                                                </div>
                                            </div>
                                            <hr>
                                            <button type="submit" class="btn btn-primary w-100">Test Gönderimi</button>
                                        </form>
                                    </div>
                                </div>
                            </div>
                            <div class="modal-footer">
                                <button type="button" class="btn btn-secondary" @click="showPreview = false">Kapat</button>
                            </div>
                        </div>
                    </div>
                </div>
            </teleport>
        </div>
    `,
    delimiters: ["[[", "]]"],
    setup(props, { emit }) {
        const { ref, computed, onMounted, watch } = Vue;

        // Inject styles
        const injectStyles = () => {
            const styleId = 'form-builder-styles';
            if (!document.getElementById(styleId)) {
                const style = document.createElement('style');
                style.id = styleId;
                style.textContent = `
                    .form-builder { color: #333; }
                    .border-dashed { border: 2px dashed #dee2e6 !important; }
                    .form-item-card { cursor: move; transition: all 0.2s; border: 1px solid #eee; margin-bottom: 10px; }
                    .form-item-card:hover { border-color: #0d6efd; box-shadow: 0 4px 12px rgba(0,0,0,0.05); }
                    .preview-mock { background: #fafafa; pointer-events: none; }
                    .x-small { font-size: 0.75rem; }
                    .italic { font-style: italic; }
                    .btn-link { text-decoration: none; }
                    .list-group-item-action:hover { background-color: #f8f9fa; border-color: #0d6efd !important; color: #0d6efd; }
                `;
                document.head.appendChild(style);
            }
        };
        injectStyles();

        const fields = ref(Array.isArray(props.modelValue) ? [...props.modelValue] : []);
        const selectedFieldId = ref(null);
        const showPreview = ref(false);
        const previewData = ref({});
        const canvasRef = ref(null);
        let fieldCounter = fields.value.length > 0 ? Math.max(...fields.value.map(f => {
            const num = parseInt(f.name.toString().replace(/\D/g, '') || 0);
            return isNaN(num) ? 0 : num;
        })) + 1 : 1;

        const selectedField = computed(() => {
            return fields.value.find(f => f.id === selectedFieldId.value);
        });

        const getTypeLabel = (type) => {
            const labels = {
                text: 'Metin',
                textarea: 'Metin Alanı',
                number: 'Sayı',
                email: 'E-posta',
                select: 'Seçim',
                radio: 'Radyo',
                checkbox: 'Onay',
                date: 'Tarih',
                file: 'Dosya'
            };
            return labels[type] || type;
        };

        const onDragStart = (e, type) => {
            e.dataTransfer.setData('fieldType', type);
        };

        const onDrop = (e) => {
            const type = e.dataTransfer.getData('fieldType');
            if (type) {
                addField(type);
            }
        };

        // --- Name Generation Helper ---
        const slugify = (text) => {
            const trMap = {
                'ç': 'c', 'ğ': 'g', 'ı': 'i', 'ö': 'o', 'ş': 's', 'ü': 'u',
                'Ç': 'c', 'Ğ': 'g', 'İ': 'i', 'Ö': 'o', 'Ş': 's', 'Ü': 'u'
            };
            let str = text.toString().toLowerCase();
            Object.keys(trMap).forEach(key => str = str.replace(new RegExp(key, 'g'), trMap[key]));
            return str.replace(/[^\w\s-]/g, '') // Remove non-word chars
                      .replace(/[\s_-]+/g, '_') // Replace spaces/underscores with _
                      .replace(/^-+|-+$/g, ''); // Remove leading/trailing -
        };

        // Watch label changes to auto-update name field if it's still default
        watch(() => selectedField.value?.label, (newLabel, oldLabel) => {
            if (selectedField.value && newLabel) {
                // If name looks like "field_123" (default) or is empty, auto-update it
                const isDefaultName = /^field_\d+$/.test(selectedField.value.name);
                if (isDefaultName || !selectedField.value.name) {
                    selectedField.value.name = slugify(newLabel);
                }
            }
        });

        const addField = (type) => {
            const defaults = {
                text: { label: 'Metin Alanı', placeholder: 'Metin giriniz...' },
                textarea: { label: 'Açıklama', placeholder: 'Detaylı bilgi giriniz...' },
                number: { label: 'Sayı', placeholder: 'Sayısal değer...' },
                email: { label: 'E-posta', placeholder: 'adiniz@domain.com' },
                select: { label: 'Seçim Yapın', options: ['Seçenek 1', 'Seçenek 2'] },
                radio: { label: 'Birini Seçin', options: ['Evet', 'Hayır'] },
                checkbox: { label: 'Onaylıyorum', options: [] },
                date: { label: 'Tarih Seçiniz', placeholder: '' },
                file: { label: 'Dosya Yükle', placeholder: '' }
            };

            const newField = {
                id: 'f_' + (new Date().getTime()),
                type,
                name: `field_${fieldCounter++}`,
                label: defaults[type].label,
                placeholder: defaults[type].placeholder || '',
                required: false,
                col: 'col-12',
                options: defaults[type].options ? [...defaults[type].options] : [],
                visibilityRules: []
            };

            fields.value.push(newField);
            selectedFieldId.value = newField.id;
        };

        const removeField = (index) => {
            if (confirm('Bu alanı silmek istediğinize emin misiniz?')) {
                const removed = fields.value.splice(index, 1)[0];
                if (selectedFieldId.value === removed.id) {
                    selectedFieldId.value = null;
                }
            }
        };

        const selectField = (field) => {
            selectedFieldId.value = field.id;
        };

        const addOption = () => {
            if (selectedField.value && selectedField.value.options) {
                selectedField.value.options.push('Yeni Seçenek');
            }
        };

        const removeOption = (index) => {
            if (selectedField.value && selectedField.value.options) {
                selectedField.value.options.splice(index, 1);
            }
        };

        const addRule = () => {
            if (selectedField.value) {
                if (!selectedField.value.visibilityRules) {
                    selectedField.value.visibilityRules = [];
                }
                selectedField.value.visibilityRules.push({
                    targetField: '',
                    values: [],
                    value: ''
                });
            }
        };

        const removeRule = (idx) => {
            selectedField.value.visibilityRules.splice(idx, 1);
        };

        const getPotentialTargetFields = () => {
            return fields.value.filter(f => f.id !== selectedFieldId.value);
        };

        const getRuleFieldOptions = (targetId) => {
            const field = fields.value.find(f => f.id === targetId);
            return (field && field.options) ? field.options : [];
        };

        const isFieldVisible = (field) => {
            if (!field.visibilityRules || field.visibilityRules.length === 0) return true;
            
            return field.visibilityRules.some(rule => {
                if (!rule.targetField) return true;
                const targetVal = previewData.value[rule.targetField];
                
                if (rule.values && rule.values.length > 0) {
                    return rule.values.includes(targetVal);
                }
                
                if (rule.value) {
                    return targetVal == rule.value;
                }
                
                return false;
            });
        };

        const clearForm = () => {
            if (confirm('Tüm form taslağı silinecektir. Emin misiniz?')) {
                fields.value = [];
                selectedFieldId.value = null;
            }
        };

        const previewSubmit = () => {
            alert('Form Datası: ' + JSON.stringify(previewData.value, null, 2));
        };

        // Sync with parent
        watch(fields, (newVal) => {
            emit('update:modelValue', JSON.parse(JSON.stringify(newVal)));
        }, { deep: true });

        // Watch prop changes
        watch(() => props.modelValue, (newVal) => {
            if (JSON.stringify(newVal) !== JSON.stringify(fields.value)) {
                fields.value = Array.isArray(newVal) ? [...newVal] : [];
            }
        }, { deep: true });

        onMounted(() => {
            if (typeof Sortable !== 'undefined' && canvasRef.value) {
                new Sortable(canvasRef.value, {
                    animation: 150,
                    handle: '.form-item-card',
                    onEnd: (evt) => {
                        const items = [...fields.value];
                        const element = items.splice(evt.oldIndex, 1)[0];
                        items.splice(evt.newIndex, 0, element);
                        fields.value = items;
                    }
                });
            }
        });

        return {
            fields,
            selectedFieldId,
            selectedField,
            showPreview,
            previewData,
            canvasRef,
            getTypeLabel,
            onDragStart,
            onDrop,
            addField,
            removeField,
            selectField,
            addOption,
            removeOption,
            addRule,
            removeRule,
            getPotentialTargetFields,
            getRuleFieldOptions,
            isFieldVisible,
            clearForm,
            previewSubmit
        };
    }
};

window.FormBuilderComponent = FormBuilderComponent;
