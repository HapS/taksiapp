import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:web_socket_channel/web_socket_channel.dart';
import '../../../core/config/app_config.dart';
import '../../auth/services/auth_service.dart';

// =============================================================================
// Server → Client WS mesaj tipleri
// =============================================================================
// Backend WS handler (handler.rs) tarafından üretilir ve passenger'lara broadcast edilir.
// Her mesaj type alanına göre ayrıştırılır (_parseMessage).

/// Temel sunucu mesajı — sealed class, tüm mesaj tiplerinin ebeveyni.
sealed class ServerMessage {}

/// Yeni sürücü teklifi (şu an passenger tarafında işlenmiyor, driver WS endpoint'inde kullanılır).
/// Backend: offer oluştuktan sonra driver'a gönderilir.
class RideOfferMessage extends ServerMessage {
  final Map<String, dynamic> data;
  RideOfferMessage(this.data);
}

/// Sürücü konum güncellemesi.
///
/// Backend: handler.rs → LocationUpdate mesajı işlendiğinde,
/// aktif ride varsa (Accepted veya PickedUp durumu)
/// hub.broadcast_driver_location() ile passenger'a gönderilir.
/// Format: { type: "driver_location", ride_id: int, lat: float, lon: float }
class DriverLocationMessage extends ServerMessage {
  final int rideId;
  final double lat;
  final double lon;
  DriverLocationMessage({required this.rideId, required this.lat, required this.lon});
}

/// Yolculuk durum değişikliği bildirimi.
///
/// Backend: ride status güncellendiğinde (cancel, complete, offer accept)
/// hub üzerinden passenger'a broadcast edilir.
/// Format: { type: "ride_status_changed", ride_id: int, status: string }
/// Olası durumlar: accepted, picked_up, completed, cancelled, no_driver
class RideStatusChangedMessage extends ServerMessage {
  final int rideId;
  final String status;
  RideStatusChangedMessage({required this.rideId, required this.status});
}

/// Sürücü teklif süresi doldu / reddedildi — yakın sürücü bulunamadı.
/// Backend: offer_expired mesajı yayınlandığında passenger durumunu 'no_driver'a çeker.
class OfferExpiredMessage extends ServerMessage {
  final int rideId;
  OfferExpiredMessage(this.rideId);
}

/// WS heartbeat yanıtı.
/// Client her 20sn'de bir ping gönderir, server pong döner.
class PongMessage extends ServerMessage {}

/// WS hata mesajı.
class ErrorMessage extends ServerMessage {
  final String message;
  ErrorMessage(this.message);
}

/// WS'den gelen raw JSON string'i ServerMessage objesine dönüştürür.
///
/// Backend mesaj formatı: { "type": `"<mesaj_tipi>"`, ...alanlar }
/// type alanına göre ilgili Dart sınıfına map edilir.
ServerMessage _parseMessage(String raw) {
  try {
    final data = jsonDecode(raw) as Map<String, dynamic>;
    final type = data['type'] as String?;
    switch (type) {
      case 'ride_offer':
        return RideOfferMessage(data);
      case 'driver_location':
        return DriverLocationMessage(
          rideId: data['ride_id'] as int,
          lat: (data['lat'] as num).toDouble(),
          lon: (data['lon'] as num).toDouble(),
        );
      case 'ride_status_changed':
        return RideStatusChangedMessage(
          rideId: data['ride_id'] as int,
          status: data['status'] as String,
        );
      case 'offer_expired':
        return OfferExpiredMessage(data['ride_id'] as int);
      case 'pong':
        return PongMessage();
      case 'error':
        return ErrorMessage(data['message'] as String? ?? 'Bilinmeyen hata');
      default:
        return ErrorMessage('Bilinmeyen mesaj tipi: $type');
    }
  } catch (e) {
    return ErrorMessage('Parse hatası: $e');
  }
}

/// WebSocket bağlantı yöneticisi — passenger tarafı.
///
/// Backend WS endpoint'i: /ws/passenger?token=`<jwt>`
/// JWT token ile kimlik doğrulama yapılır. Backend handler.rs
/// verify_ws_token() ile token'ı doğrular, user_id'yi çıkarır.
///
/// Bağlantı lifecycle:
/// 1. connect() ile JWT token alınır ve WS bağlantısı açılır
/// 2. Gelen mesajlar StreamController ile broadcast edilir
/// 3. rideProvider _handleWsMessage ile mesajları dinler
/// 4. disconnect() ile bağlantı kapatılır
///
/// Ping mekanizması: Her 20sn'de bir { "type": "ping" } gönderilir,
/// server { "type": "pong" } yanıt döner. Bu bağlantının canlı tutulmasını sağlar.
class RideWsService {
  WebSocketChannel? _channel;
  StreamController<ServerMessage>? _messageController;
  Timer? _pingTimer;
  final _authService = AuthService();

  /// WS'den gelen mesajların broadcast stream'i.
  /// rideProvider bu stream'i dinleyerek sürücü konumu ve durum değişikliklerini alır.
  Stream<ServerMessage> get messages => _messageController!.stream;

  /// WebSocket bağlantısını JWT token ile açar.
  ///
  /// Token'ı FlutterSecureStorage'dan alır, WS URL'ine query param olarak ekler.
  /// Backend: /ws/passenger?token=`<jwt>`
  /// handler.rs'de verify_ws_token() ile JWT doğrulanır.
  /// Token geçersizse bağlantı kurulmaz.
  Future<void> connect() async {
    disconnect();

    _messageController = StreamController<ServerMessage>.broadcast();

    // JWT token'ı al
    final token = await _authService.getAccessToken();
    if (token == null) {
      debugPrint('RideWsService: token bulunamadı, bağlantı iptal');
      _messageController?.add(ErrorMessage('Token bulunamadı'));
      return;
    }

    final uri = Uri.parse('${AppConfig.wsBaseUrl}/ws/passenger?token=${Uri.encodeComponent(token)}');
    debugPrint('RideWsService: connecting to ${uri.host}${uri.path}');

    _channel = WebSocketChannel.connect(uri);

    try {
      await _channel!.ready;
      debugPrint('RideWsService: connected');
    } catch (e) {
      debugPrint('RideWsService: connection failed: $e');
      _messageController?.add(ErrorMessage('Bağlantı kurulamadı: $e'));
      return;
    }

    _channel!.stream.listen(
      (data) {
        debugPrint('══════ WS GELEN ══════');
        debugPrint('$data');
        debugPrint('══════════════════════');
        final msg = _parseMessage(data as String);
        _messageController?.add(msg);
      },
      onError: (error) {
        debugPrint('RideWsService: error: $error');
        _messageController?.add(ErrorMessage(error.toString()));
      },
      onDone: () {
        debugPrint('RideWsService: connection closed');
      },
    );

    startPingTimer();
  }

  /// WS heartbeat ping timer'ını başlatır.
  /// Her 20 saniyede bir { "type": "ping" } mesajı gönderir.
  /// Backend WS handler ping mesajını karşılar ve pong döner.
  void startPingTimer() {
    _pingTimer?.cancel();
    _pingTimer = Timer.periodic(const Duration(seconds: 20), (_) {
      _send({'type': 'ping'});
    });
  }

  /// Sürücü konum güncellemesi gönderir (passenger tarafında kullanılmaz, driver WS endpoint'inde kullanılır).
  /// Backend: handler.rs → LocationUpdate mesajı olarak işlenir
  void sendLocationUpdate(double lat, double lon) {
    _send({'type': 'location_update', 'lat': lat, 'lon': lon});
  }

  /// Sürücü teklif yanıtı gönderir (passenger tarafında kullanılmaz, driver WS endpoint'inde kullanılır).
  /// Backend: handler.rs → OfferResponse mesajı olarak işlenir
  void sendOfferResponse(int rideId, bool accepted) {
    _send({'type': 'offer_response', 'ride_id': rideId, 'accepted': accepted});
  }

  /// WS kanalına JSON mesaj gönderir.
  void _send(Map<String, dynamic> data) {
    try {
      final json = jsonEncode(data);
      debugPrint('══════ WS GÖNDERİLEN ══════');
      debugPrint('$json');
      debugPrint('══════════════════════════');
      _channel?.sink.add(json);
    } catch (e) {
      debugPrint('RideWsService: send error: $e');
    }
  }

  /// WebSocket bağlantısını ve kaynakları temizler.
  /// Ping timer durdurulur, kanal kapatılır, stream controller dispose edilir.
  void disconnect() {
    _pingTimer?.cancel();
    _pingTimer = null;
    _channel?.sink.close();
    _channel = null;
    _messageController?.close();
    _messageController = null;
    debugPrint('RideWsService: disconnected');
  }
}