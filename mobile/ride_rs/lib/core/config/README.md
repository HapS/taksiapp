# Uygulama Yapılandırması

Bu klasör, uygulama genelinde kullanılan yapılandırma ayarlarını içerir.

## AppConfig

`app_config.dart` dosyası, tüm uygulama ayarlarını merkezi bir yerden yönetir.

### Kullanım

```dart
import 'package:ride_rs/core/config/app_config.dart';

// API çağrıları için
final response = await http.get(Uri.parse('${AppConfig.apiEndpoint}/products'));

// Medya URL'leri için
final imageUrl = '${AppConfig.mediaBaseUrl}${product.imageUrl}';

// Diğer ayarlar
final language = AppConfig.defaultLanguage;
final timeout = AppConfig.apiTimeout;
```

### Yapılandırma Değişkenleri

- **apiBaseUrl**: Ana API URL'i (örn: `http://192.168.1.2:3000`)
- **apiEndpoint**: API endpoint'i (örn: `http://192.168.1.2:3000/api`)
- **mediaBaseUrl**: Medya dosyaları için base URL
- **apiTimeout**: API istekleri için timeout süresi (saniye)
- **productsPerPage**: Sayfa başına gösterilecek ürün sayısı
- **defaultLanguage**: Varsayılan dil kodu
- **supportedLanguages**: Desteklenen diller listesi
- **isDebugMode**: Debug modu aktif mi?

### Cache Yönetimi

Uygulama cache kullanmaz. Tüm istekler her seferinde sunucudan çekilir.

- HTTP istekleri: `Cache-Control: no-cache` header'ı ile
- Image'lar: `Cache-Control: no-cache` header'ı ile
- Image cache temizleme: `CacheManager.clearImageCache()` kullanın

### Ortam Değişikliği

Farklı ortamlar (development, staging, production) için URL'leri değiştirmek isterseniz:

1. `app_config.dart` dosyasını açın
2. `apiBaseUrl` değerini güncelleyin
3. Uygulamayı yeniden derleyin

**Örnek:**

```dart
// Development
static const String apiBaseUrl = 'http://192.168.1.2:3000';

// Production
static const String apiBaseUrl = 'https://api.yourapp.com';
```

### İleri Seviye: Ortam Bazlı Yapılandırma

Gelecekte farklı ortamlar için ayrı config dosyaları oluşturabilirsiniz:

```
lib/core/config/
  ├── app_config.dart          # Ana config interface
  ├── dev_config.dart          # Development ayarları
  ├── staging_config.dart      # Staging ayarları
  └── prod_config.dart         # Production ayarları
```

Ve build time'da hangisinin kullanılacağını belirleyebilirsiniz.
