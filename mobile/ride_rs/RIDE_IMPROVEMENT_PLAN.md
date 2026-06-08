# Ride Modülü İyileştirme Planı

> Tarih: 2026-05-23
> Mevcut durum: MVP çalışıyor, üretime geçiş için iyileştirmeler planlanıyor.
> Ödeme: Uygulama üzerinden ödeme alınmayacak. Yolcu ile taksici arasında nakit/kart ile ödeme yapılacak. Biz sadece eşleştirme hizmeti veriyoruz.

---

## 🔴 Flutter — Kritik UX Hataları (Önce Bunlar)

### 1. X butonu aktif yolculuğu iptal ediyor
- **Dosya**: `lib/modules/ride_sharing/home_page.dart:462-466`
- **Sebep**: `suffixIcon` içindeki `IconButton` her durumda `clearRoute()` çağırıyor
- **`clearRoute()`** (`ride_provider.dart:113-121`): `rideStatus != 'idle'` ise WS bağlantısını kapatıyor, polling'i durduruyor, state'i sıfırlıyor — yani aktif yolculuğu iptal ediyor
- **Çözüm**: X butonu sadece `rideStatus == 'idle'` iken göster:
  ```dart
  suffixIcon: rideState.rideStatus == 'idle' && _destinationController.text.isNotEmpty
      ? IconButton(icon: const Icon(Icons.clear), onPressed: () { ... })
      : null,
  ```

### 2. accepted/picked_up durumunda iptal butonu yok
- **Dosya**: `lib/modules/ride_sharing/home_page.dart:635-667`
- **Sebep**: `_buildRideStatusCard`'da `searching` case'inde iptal butonu var ama `accepted`/`picked_up` case'inde yok
- **Çözüm**: Sürücü bilgisi kartının altına kırmızı "İptal" butonu ve sürücüyü arama butonu ekle:
  ```dart
  Row(
    children: [
      TextButton.icon(
        onPressed: () => ref.read(rideProvider.notifier).cancelRide(),
        icon: const Icon(Icons.cancel, color: Colors.red),
        label: const Text('İptal', style: TextStyle(color: Colors.red)),
      ),
      const SizedBox(width: 8),
      TextButton.icon(
        onPressed: () => launchUrl('tel:${driver?.phone}'),
        icon: const Icon(Icons.phone, color: Colors.green),
        label: Text('Ara', style: TextStyle(color: Colors.green)),
      ),
    ],
  )
  ```
  Not: `cancelRide()` şu an sadece lokal state sıfırlıyor. Backend'e iptal bildirimi için endpoint eklenecek (madde 8).

### 3. completed string concatenation
- **Dosya**: `lib/modules/ride_sharing/home_page.dart:693-694`
- **Hata**: `'Yolculuk tamamlandı' '${...}'` — Dart'ta yan yana string literaller birleşir ama okunması zor
- **Çözüm**: Tek bir string yap:
  ```dart
  Text(
    'Yolculuk tamamlandı${rideState.fareAmount != null ? ' • ${rideState.fareAmount!.toStringAsFixed(2)} TL' : ''}',
  ),
  ```

---

## 🟡 Flutter — İyileştirmeler

### 4. accepted/picked_up'ta rota gösterimi
- Şu an sadece pickup→dropoff rotası (`rideState.routePoints`) çiziliyor
- Eşleşme sonrası **sürücü→pickup** rotası da farklı renkte gösterilmeli (ör: yeşil)
- `RideState`'e `driverToPickupPoints: List<LatLng>` alanı eklenebilir
- ORS'den backend üzerinden alınacak (madde 8)

### 5. Arama input'u yolculuk sırasında kilitlenmeli
- **Dosya**: `home_page.dart:454`
- Kullanıcı yolculuk devam ederken yeni adres arayabiliyor
- **Çözüm**: `readOnly: rideState.rideStatus != 'idle'`
- Aktif yolculukta input görünür ama pasif olmalı

### 6. Sürücü konumu güncellenince harita kamerası bozuluyor (zoom + pozisyon sıfırlanıyor)
- **Dosya**: `home_page.dart:217-222`
- **Sebep**: `ref.listen` her `driverLocation` değişiminde `_mapController.move(next.driverLocation!, 14)` çağırıyor
  - Bu, zoom'u sabit `14`'e çekiyor — kullanıcı yakınlaştırdıysa sıfırlanıyor
  - Kamerayı sürücünün üstüne ortuluyor — kullanıcı farklı bir yere bakıyorsa zorla taşınıyor
  - Fake driver 2sn'de bir, gerçek sürücü 3sn'de bir konum gönderiyor → sürekli kamera sıçrıyor
- **Çözüm**: Kullanıcı müdahalesini tanı, sadece "takip modu" açıkken otomatik kaydır:
  ```dart
  bool _cameraFollowing = true;

  // Kullanıcı haritayı manuel kaydırınca takibi kapat
  MapOptions(
    onPositionChanged: (pos, hasGesture) {
      if (hasGesture) _cameraFollowing = false;
    },
  )

  // Sürücü konumu gelince sadece takip modu açıkken move et
  ref.listen<RideState>(rideProvider, (prev, next) {
    if (next.driverLocation != null &&
        next.driverLocation != prev?.driverLocation &&
        _cameraFollowing) {
      _mapController.move(next.driverLocation!, _mapController.camera.zoom);
    }
  });

  // Tekrar takibe al: "Konumum" butonuna basınca
  onPressed: () {
    _cameraFollowing = true;
    _mapController.move(rideState.currentLocation!, 15);
  }
  ```
- Ayrıca, `_mapController.move()` yerine `_mapController.fitCamera()` kullanılırsa zoom korunmaz. Mevcut zoom'u koruyarak sadece konumu güncellemek daha iyi.
- `picked_up` sonrası sürücüyü takip etmek mantıklı, `accepted`'ta sadece marker güncellenmeli (kamera takibi yapılmamalı).

### 7. "Tekrar Dene"/"Kapat" butonları standartlaştırılmalı
- `no_driver` ve `completed` durumlarında kullanılan buton stilleri tutarlı değil
- `no_driver`: "Kapat" + "Tekrar Dene"
- `completed`: "Kapat"
- Tüm kartlarda aynı buton stili kullanılmalı

---

## 🔧 Backend — Yeni Endpoint'ler

### 8. ORS routing backend'e taşı
- **Dosya**: `src/modules/ride/controllers/ride.rs:87-94`
- Şu an `fetch_ors_route()` → `(None, None)` dönüyor
- Mobil `RouteService` ORS'yi doğrudan çağırıyor — bu backend'e taşınmalı
- **Plan**:
  - `config.toml`'a `[ors]` bölümü ekle: `api_key`, `base_url`
  - `fetch_ors_route()`'u gerçek ORS API çağrısı ile doldur
  - `POST /api/ride/request` sırasında backend ORS'den route alır:
    - `distance_km`, `duration_sec` hesaplanır
    - `fare_amount` opsiyonel olarak hesaplanır (opsiyonel, ödeme bizde değil)
    - Opsiyonel: polyline dönülebilir
  - Mobil `RouteService.getRoute()` kaldırılır, backend'den alınır

### 9. Status geçiş endpoint'leri
Backend'de şu an sadece `request` ve `get` var. Eksikler:

| Endpoint | Method | İşlev | Body |
|----------|--------|-------|------|
| `/api/ride/{id}/status` | POST | Sürücü durum değiştirir | `{ "status": "picked_up" | "completed" }` |
| `/api/ride/{id}/cancel` | POST | Yolcu/sürücü iptal eder | `{ "by": "passenger" | "driver" }` |

- `picked_up`: `driver_id` kontrolü yap, ride'ı güncelle
- `completed`: picked_up sonrası geçerli, timestamp ata, yolcuya WS bildir
- `cancelled`: searching/accepted/picked_up durumlarında geçerli

### 10. Fare hesaplama (opsiyonel, ödeme bizde olmadığı için)
- **Dosya**: `dispatch.rs:88-91` — `fare_amount = 0.0`
- Ödeme bizde olmasa bile tahmini ücret göstermek kullanıcı deneyimi için faydalı
- Basit formül: `base_fare + (distance_km * per_km_rate)`
- Config'den okunur: `config.toml` → `[ride_pricing]`
- `POST /api/ride/request` response'una eklenir

### 11. WebSocket — Sürücü tarafı da yolcuyu arayabilmeli
- Sürücü WS'den yolcu konumunu görmeli
- `ride_rooms`'a sürücü→yolcu mesajlaşması için kanal eklenebilir

---

## 🗄️ Veritabanı Değişiklikleri

### Yeni migration gerekebilir:
```sql
-- rides tablosuna ek alanlar
ALTER TABLE rides ADD COLUMN cancelled_by VARCHAR(20);  -- 'passenger', 'driver'
ALTER TABLE rides ADD COLUMN cancellation_reason TEXT;

-- rating tablosu (opsiyonel, sonra)
CREATE TABLE ride_ratings (
    id          BIGSERIAL PRIMARY KEY,
    ride_id     BIGINT NOT NULL REFERENCES rides(id),
    from_user   BIGINT NOT NULL REFERENCES users(id),  -- yolcu
    to_driver   BIGINT NOT NULL REFERENCES drivers(id),
    rating      INTEGER NOT NULL CHECK (rating >= 1 AND rating <= 5),
    comment     TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

---

## 🚗 Yolculuk Sonrası Akış (Ödemesiz)

Ödeme bizde olmadığı için sadeleştirilmiş flow:

```
COMPLETED
  ├─ "Yolculuk tamamlandı" mesajı
  ├─ Mesafe ve süre bilgisi göster
  ├─ "Sürücüye nakit/kart ile ödeme yapınız" uyarısı
  ├─ Sürücüyü puanla (1-5 yıldız) — opsiyonel
  └─ "Kapat" → IDLE
```

**Flutter'da yapılacak**:
- `_buildRideStatusCard` → `completed` case'inde ücret yerine "Nakit/Kart ile ödemeyi sürücüye yapın" mesajı
- Puanlama için bottom sheet (isteğe bağlı)
- `resetRide()` sonrası rota bilgisi korunur, tekrar çağırmak kolay

---

## 📋 Öncelik Sırası

| Öncelik | Ne | Nerede |
|---------|---|--------|
| 1 | X butonu hatası | Flutter `home_page.dart` |
| 2 | accepted/picked_up iptal butonu | Flutter `home_page.dart` |
| 3 | completed string bug | Flutter `home_page.dart` |
| 4 | Arama input'u kilitleme | Flutter `home_page.dart` |
| 5 | ORS routing backend'e taşı | Backend `controllers/ride.rs` |
| 6 | Status endpoint'leri (picked_up, completed) | Backend yeni endpoint |
| 7 | Cancel endpoint | Backend yeni endpoint |
| 8 | accepted/picked_up'ta rota gösterimi | Flutter `home_page.dart` + Provider |
| 9 | Fare hesaplama (opsiyonel) | Backend `dispatch.rs` |
| 10 | Yolculuk sonrası puanlama | Flutter + Backend |
