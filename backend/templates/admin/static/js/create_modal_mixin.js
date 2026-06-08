// Modal-specific Vue component mixin
const createCreditModalMixin = {
  data() {
    return {
      // User search
      userSearch: '',
      userSearchResults: [],
      userSearchLoading: false,
      showUserDropdown: false,
      selectedUser: null,
      userSearchTimeout: null,

      // Form data
      createForm: {
        user_id: null,
        credit_type: 'admin_gift',
        amount: null,
        currency: 'TRY',
        min_order_amount: null,
        valid_from: '',
        valid_until: '',
        description: '',
        admin_note: ''
      },

      // Validation
      amountError: '',
      dateError: '',
      submitting: false
    };
  },

  computed: {
    isFormValid() {
      return (
        this.createForm.user_id &&
        this.createForm.credit_type &&
        this.createForm.amount > 0 &&
        this.createForm.currency &&
        this.createForm.valid_from &&
        this.createForm.valid_until &&
        this.createForm.description &&
        !this.amountError &&
        !this.dateError
      );
    }
  },

  methods: {
    // User search functionality
    async searchUsers() {
      const query = this.userSearch.trim();
      
      if (query.length < 2) {
        this.userSearchResults = [];
        return;
      }

      clearTimeout(this.userSearchTimeout);
      this.userSearchTimeout = setTimeout(async () => {
        this.userSearchLoading = true;
        try {
          const response = await fetch(`/admin/api/users?search=${encodeURIComponent(query)}&limit=10`);
          const data = await response.json();
          
          if (response.ok && data.data) {
            this.userSearchResults = data.data;
          } else {
            this.userSearchResults = [];
          }
        } catch (error) {
          console.error('Kullanıcı araması başarısız:', error);
          this.userSearchResults = [];
        } finally {
          this.userSearchLoading = false;
        }
      }, 300);
    },

    selectUser(user) {
      this.selectedUser = user;
      this.createForm.user_id = user.id;
      this.userSearch = user.email;
      this.showUserDropdown = false;
      this.userSearchResults = [];
    },

    // Validation methods
    validateAmount() {
      const amount = parseFloat(this.createForm.amount);
      if (isNaN(amount) || amount <= 0) {
        this.amountError = 'Tutar 0\'dan büyük olmalıdır';
      } else {
        this.amountError = '';
      }
    },

    validateDates() {
      const now = new Date();
      const validFrom = new Date(this.createForm.valid_from);
      const validUntil = new Date(this.createForm.valid_until);

      if (validUntil <= validFrom) {
        this.dateError = 'Geçerlilik Bitişi, Geçerlilik Başlangıcından sonra olmalıdır';
        return false;
      }

      if (validUntil <= now) {
        this.dateError = 'Geçerlilik Bitişi gelecekte olmalıdır';
        return false;
      }

      this.dateError = '';
      return true;
    },

    // Form submission
    async createCredit() {
      if (!this.isFormValid) {
        return;
      }

      if (!this.validateDates()) {
        return;
      }

      this.submitting = true;

      try {
        const payload = {
          user_id: parseInt(this.createForm.user_id),
          credit_type: this.createForm.credit_type,
          amount: parseFloat(this.createForm.amount),
          currency: this.createForm.currency,
          min_order_amount: this.createForm.min_order_amount ? parseFloat(this.createForm.min_order_amount) : null,
          valid_from: new Date(this.createForm.valid_from).toISOString(),
          valid_until: new Date(this.createForm.valid_until).toISOString(),
          description: this.createForm.description,
          admin_note: this.createForm.admin_note || null
        };

        const response = await fetch('/admin/api/user-credits', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });

        const data = await response.json();

        if (response.ok) {
          Swal.fire('Başarılı', 'Kredi başarıyla oluşturuldu', 'success');
          bootstrap.Modal.getInstance(document.getElementById('createCreditModal')).hide();
          
          // Trigger reload in parent component
          if (this.loadCredits) {
            this.loadCredits();
          }
          
          this.resetCreateForm();
        } else {
          Swal.fire('Hata', data.error || 'Kredi oluşturulamadı', 'error');
        }
      } catch (error) {
        console.error('Kredi oluşturulamadı:', error);
        Swal.fire('Hata', 'Kredi oluşturulamadı', 'error');
      } finally {
        this.submitting = false;
      }
    },

    // Form reset
    resetCreateForm() {
      this.createForm = {
        user_id: null,
        credit_type: 'admin_gift',
        amount: null,
        currency: 'TRY',
        min_order_amount: null,
        valid_from: '',
        valid_until: '',
        description: '',
        admin_note: ''
      };
      this.userSearch = '';
      this.selectedUser = null;
      this.userSearchResults = [];
      this.amountError = '';
      this.dateError = '';
      this.initializeDefaultDates();
    },

    // Initialize default dates
    initializeDefaultDates() {
      const now = new Date();
      const nextMonth = new Date(now);
      nextMonth.setMonth(nextMonth.getMonth() + 1);
      
      this.createForm.valid_from = this.formatDateTimeForInput(now);
      this.createForm.valid_until = this.formatDateTimeForInput(nextMonth);
    },

    formatDateTimeForInput(date) {
      const year = date.getFullYear();
      const month = String(date.getMonth() + 1).padStart(2, '0');
      const day = String(date.getDate()).padStart(2, '0');
      const hours = String(date.getHours()).padStart(2, '0');
      const minutes = String(date.getMinutes()).padStart(2, '0');
      return `${year}-${month}-${day}T${hours}:${minutes}`;
    },

    // Show modal
    showCreateModal() {
      this.resetCreateForm();
      const modal = new bootstrap.Modal(document.getElementById('createCreditModal'));
      modal.show();
    }
  },

  mounted() {
    // Close dropdown when clicking outside
    document.addEventListener('click', (e) => {
      if (!e.target.closest('.position-relative')) {
        this.showUserDropdown = false;
      }
    });

    // Initialize default dates
    this.initializeDefaultDates();
  }
};
