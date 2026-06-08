// Vue Components for Content Editor

// CategoryItem Component - Recursive category tree
const CategoryItemComponent = {
    name: "CategoryItem",
    delimiters: ["[[", "]]"],
    props: {
        term: Object,
        selectedIds: Array,
        level: {
            type: Number,
            default: 0,
        },
    },
    emits: ["toggle"],
    template: `
        <div class="mb-2">
          <div class="form-check" :style="{ marginLeft: (level * 20) + 'px' }">
            <input
              class="form-check-input"
              type="checkbox"
              :value="term.id"
              :checked="selectedIds.includes(term.id)"
              @change="$emit('toggle', term.id)"
              :id="\`term-\${term.id}\`"
            >
            <label class="form-check-label small" :for="\`term-\${term.id}\`">
              <span v-if="level === 0" class="fw-semibold" :class="{ 'text-danger opacity-50': term.publish === false }">
                <i v-if="term.publish === false" class="bi bi-eye-slash me-1 text-danger"></i>
                <i v-else class="bi bi-folder text-primary"></i> [[ term.title ]]
              </span>
              <span v-else :class="{ 'text-danger opacity-50': term.publish === false }">
                <i v-if="term.publish === false" class="bi bi-eye-slash me-1 text-danger"></i>
                <i v-else class="bi bi-arrow-return-right text-muted me-1"></i> [[ term.title ]]
              </span>
            </label>
          </div>

          <!-- Alt kategoriler recursive -->
          <div v-if="term.children && term.children.length > 0">
            <category-item
              v-for="childTerm in term.children"
              :key="childTerm.id"
              :term="childTerm"
              :selected-ids="selectedIds"
              @toggle="$emit('toggle', $event)"
              :level="level + 1"
            ></category-item>
          </div>
        </div>
    `,
};

// CategoryOption Component - Recursive option rendering for select dropdown
const CategoryOptionComponent = {
    name: "CategoryOption",
    props: {
        term: Object,
        level: {
            type: Number,
            default: 0,
        },
    },
    setup(props) {
        const indentPrefix = Vue.computed(() => {
            return "—".repeat(props.level) + (props.level > 0 ? " " : "");
        });

        return { indentPrefix };
    },
    template: `
        <option :value="term.id">[[ indentPrefix ]][[ term.title ]]</option>
        <category-option
          v-for="childTerm in term.children"
          v-if="term.children && term.children.length > 0"
          :key="childTerm.id"
          :term="childTerm"
          :level="level + 1"
        ></category-option>
    `,
    delimiters: ["[[", "]]"],
};
