import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

/// Adres çözümleme (Geocoding) servisi — Nominatim (OpenStreetMap) API kullanır.
///
/// Bu servis backend ile ilişkili değildir; doğrudan Nominatim API'ye istek atar.
/// Kullanıcı bir harita noktasına dokunduğunda veya adres girdiğinde
/// koordinat ↔ adres dönüşümü için kullanılır.
///
/// Nominatim kullanım politikası gereği User-Agent header'ı zorunludur.
class GeocodingService {
  static const String _baseUrl = 'https://nominatim.openstreetmap.org';
  static const String _userAgent = 'ride_rs/1.0'; // Nominatim politikası gereği

  /// Adres metnini koordinatlara çevirir (Forward Geocoding).
  ///
  /// [address] — kullanıcı dostu adres metni (ör: "Sakarya Üniversitesi")
  /// Dönüş: LatLng — adresin koordinatı
  ///
  /// Nominatim REST API: GET /search?format=json&q=`<address>`
  /// İlk sonucun enlem/boylam değerleri parse edilir.
  static Future<LatLng> geocode(String address) async {
    final encodedAddress = Uri.encodeComponent(address);
    final url = Uri.parse('$_baseUrl/search?format=json&q=$encodedAddress');

    try {
      final response = await http.get(url, headers: {'User-Agent': _userAgent});

      if (response.statusCode == 200) {
        final data = json.decode(response.body);

        if (data != null && data.isNotEmpty) {
          final lat = double.parse(data[0]['lat']);
          final lon = double.parse(data[0]['lon']);
          return LatLng(lat, lon);
        } else {
          throw Exception('Adres bulunamadı');
        }
      } else {
        throw Exception('Geocoding hatası: ${response.statusCode}');
      }
    } catch (e) {
      throw Exception('Geocoding servisi hatası: $e');
    }
  }

  /// Koordinatları adrese çevirir (Reverse Geocoding).
  ///
  /// [lat] — enlem
  /// [lon] — boylam
  /// Dönüş: Adres string'i (ör: "Serdivan, Sakarya, Türkiye")
  ///
  /// Nominatim REST API: GET /reverse?format=json&lat=`<lat>`&lon=`<lon>`
  /// display_name alanı okunur.
  static Future<String> reverseGeocode(double lat, double lon) async {
    final url = Uri.parse('$_baseUrl/reverse?format=json&lat=$lat&lon=$lon');

    try {
      final response = await http.get(url, headers: {'User-Agent': _userAgent});

      if (response.statusCode == 200) {
        final data = json.decode(response.body);

        if (data != null && data['display_name'] != null) {
          return data['display_name'];
        } else {
          throw Exception('Adres bulunamadı');
        }
      } else {
        throw Exception('Reverse geocoding hatası: ${response.statusCode}');
      }
    } catch (e) {
      throw Exception('Reverse geocoding servisi hatası: $e');
    }
  }
}