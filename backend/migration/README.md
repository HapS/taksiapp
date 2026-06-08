# Migration

Bu klasör SeaORM migration dosyalarını içerir.

## Yapı

- `src/m20260112_000001_initial_schema.rs` - Tüm tabloları içeren tek migration dosyası
- `src/legacy_migrations/` - Eski parçalı migration dosyaları (yedek olarak saklanıyor)

## Tablolar

Migration dosyası aşağıdaki tabloları oluşturur:

### 1. Users & Auth
- `users` - Kullanıcı bilgileri
- `sessions` - Oturum verileri

### 2. RBAC (Role-Based Access Control)
- `roles` - Roller
- `permissions` - İzinler
- `role_permissions` - Rol-izin ilişkisi
- `user_roles` - Kullanıcı-rol ilişkisi
- `user_permissions` - Kullanıcı-izin ilişkisi (direkt)

### 3. Content Management
- `contents` - İçerikler (page, product, vb.)
- `vocabularies` - Taksonomi grupları
- `terms` - Taksonomi terimleri
- `content_terms` - İçerik-terim ilişkisi
- `vocabulary_categories` - Vocabulary-kategori ilişkisi

### 4. Media
- `media` - Medya dosyaları

### 5. Address
- `countries` - Ülkeler
- `cities` - Şehirler
- `districts` - İlçeler
- `addresses` - Kullanıcı adresleri
- `corporate_infos` - Kurumsal fatura bilgileri

### 6. E-Commerce
- `carts` - Sepetler/Siparişler
- `cart_items` - Sepet ürünleri

### 7. Other
- `timeline_events` - Zaman çizelgesi olayları
- `settings` - Sistem ayarları
- `mail_queue` - Mail kuyruğu
- `exchange_rates` - Döviz kurları
- `homepage` - Anasayfa yapılandırması

## Kullanım

### Yeni veritabanı oluşturma
```bash
# Migration'ı çalıştır
DATABASE_URL="postgresql://user:pass@localhost:5432/dbname" sea-orm-cli migrate up
```

### Migration durumunu kontrol etme
```bash
DATABASE_URL="postgresql://user:pass@localhost:5432/dbname" sea-orm-cli migrate status
```

## Not

Bu migration dosyası 12 Ocak 2026 tarihinde mevcut veritabanı yapısından oluşturulmuştur.
Eski parçalı migration dosyaları `legacy_migrations/` klasöründe yedek olarak saklanmaktadır.


sea-orm-cli ürettiği migration dosyası kötü berbat bozuk kodlar üretiyor. Yeni migration dosyası oluşturmak için aşağıdaki adımları izleyin:

```bash
cd migrations
export DATABASE_URL="postgres://postgres:as45dfck@localhost/backend_rs"
cargo run -p migration -- generate vocab_and_term_hide_lock_field




entitiy üretimi 
sea-orm-cli generate entity -u $DATABASE_URL -o src/entities --tables kargo_sirketleri




//tüm dabase için
cargo run -p entity -- generate all --database-url $DATABASE_URL -o src/entity
```
