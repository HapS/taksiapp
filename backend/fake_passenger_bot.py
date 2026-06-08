#!/usr/bin/env python3
"""
Fake Yolcu Botu
- Backend'e login olur, JWT alır
- POST /api/ride/request ile taksi çağırır (JWT Auth gerekli, user_id body'de değil)
- WS /ws/passenger?token=<jwt> ile bağlanır (JWT ile kimlik doğrulama)
- Sürücü kabul edince bekler, completed olunca tekrar çağırır
- 4 paralel yolcu simüle eder
"""

import asyncio
import json
import random
import signal
import sys
import urllib.error
import urllib.parse
import urllib.request

import websockets

# --- Ayarlar ---
API_BASE = "https://one.web.tr/api"
WS_BASE = "wss://one.web.tr"

# Test yolcuları: (username, password, pickup bölgesi merkezi)
PASSENGERS = [
    ("yaman.turner.1", "123456ck", (40.780591, 30.399207)),
    ("kerem.gul.2", "123456ck", (40.771207, 30.371579)),
    ("goktug.hall.3", "123456ck", (40.750287, 30.418926)),
    ("zoe.scott.4", "123456ck", (40.711464, 30.365059)),
    ("max.lewis.5", "123456ck", (40.688300, 30.269469)),
    ("yasemin.caliskan.6", "123456ck", (40.761559, 30.441726)),
    ("ece.king.7", "123456ck", (40.794396, 30.357615)),
    ("lily.carter.8", "123456ck", (40.754990, 30.387786)),
    ("funda.durmus.9", "123456ck", (40.741019, 30.438582)),
    ("hugo.baker.10", "123456ck", (40.808617, 30.409173)),
    ("kate.hall.11", "123456ck", (40.688810, 30.346373)),
    ("tanju.hill.12", "123456ck", (40.722868, 30.393317)),
    ("ben.hill.13", "123456ck", (40.775154, 30.426942)),
    ("sara.tas.14", "123456ck", (40.748468, 30.357529)),
    ("songul.hall.15", "123456ck", (40.779446, 30.402653)),
    ("simge.durmus.16", "123456ck", (40.771111, 30.373979)),
    ("ece.wright.17", "123456ck", (40.750382, 30.416502)),
    ("berke.wright.18", "123456ck", (40.711477, 30.362648)),
    ("max.kose.19", "123456ck", (40.693443, 30.269421)),
    ("mike.korkmaz.20", "123456ck", (40.763513, 30.445684)),
    ("mert.aksu.21", "123456ck", (40.794625, 30.353398)),
    ("sevgi.gul.22", "123456ck", (40.760355, 30.391592)),
    ("arda.tas.23", "123456ck", (40.737953, 30.435595)),
    ("hugo.kilic.24", "123456ck", (40.808202, 30.413001)),
    ("alp.lopez.25", "123456ck", (40.686836, 30.348304)),
    ("ece.kose.26", "123456ck", (40.723841, 30.391945)),
    ("daisy.schmidt.27", "123456ck", (40.777345, 30.432057)),
    ("alp.schmidt.28", "123456ck", (40.747216, 30.354912)),
    ("ege.young.29", "123456ck", (40.778314, 30.403346)),
    ("burcu.mitchell.30", "123456ck", (40.772117, 30.375584)),
    ("berke.yalcin.31", "123456ck", (40.753004, 30.418510)),
    ("tanju.lopez.32", "123456ck", (40.716562, 30.365755)),
    ("mike.guner.33", "123456ck", (40.692639, 30.269650)),
    ("utku.durmus.34", "123456ck", (40.767720, 30.448557)),
    ("kemal.karadeniz.35", "123456ck", (40.794793, 30.356313)),
    ("mark.kose.36", "123456ck", (40.754485, 30.391612)),
    ("derya.aksu.37", "123456ck", (40.741177, 30.438945)),
    ("tuba.roberts.38", "123456ck", (40.810575, 30.408277)),
    ("tuba.durmus.39", "123456ck", (40.684086, 30.351349)),
    ("mark.tas.40", "123456ck", (40.716181, 30.394694)),
    ("eren.lewis.41", "123456ck", (40.775344, 30.426937)),
    ("zoe.king.42", "123456ck", (40.746472, 30.357146)),
    ("daisy.lewis.43", "123456ck", (40.779035, 30.399981)),
    ("lale.adams.44", "123456ck", (40.769128, 30.377971)),
    ("simge.lopez.45", "123456ck", (40.746645, 30.419593)),
    ("arda.roberts.46", "123456ck", (40.715396, 30.368067)),
    ("lale.young.47", "123456ck", (40.694554, 30.272912)),
    ("doruk.green.48", "123456ck", (40.763227, 30.444322)),
    ("alp.mitchell.49", "123456ck", (40.793870, 30.358074)),
    ("yasemin.yavuz.50", "123456ck", (40.761662, 30.387207)),
    ("alp.hall.51", "123456ck", (40.737410, 30.432856)),
    ("firat.gunduz.52", "123456ck", (40.805867, 30.409880)),
    ("mert.clark.53", "123456ck", (40.685713, 30.348102)),
    ("iris.gunduz.54", "123456ck", (40.716033, 30.394352)),
    ("utku.tekin.55", "123456ck", (40.776954, 30.430531)),
    ("iris.korkmaz.56", "123456ck", (40.753625, 30.356524)),
    ("damla.kilic.57", "123456ck", (40.782124, 30.402941)),
    ("jack.ozkan.58", "123456ck", (40.771410, 30.371432)),
    ("gizem.lewis.59", "123456ck", (40.753196, 30.422240)),
    ("kate.caliskan.60", "123456ck", (40.717996, 30.367383)),
    ("pinar.eren.62", "123456ck", (40.691139, 30.269192)),
    ("damla.young.63", "123456ck", (40.761828, 30.446074)),
    ("ben.baker.64", "123456ck", (40.791498, 30.351539)),
    ("ece.clark.65", "123456ck", (40.755670, 30.387298)),
    ("evren.lopez.66", "123456ck", (40.738720, 30.431421)),
    ("selin.mitchell.67", "123456ck", (40.804002, 30.407210)),
    ("toprak.hall.68", "123456ck", (40.681812, 30.348909)),
    ("ece.yalcin.69", "123456ck", (40.716204, 30.397995)),
    ("derya.gul.70", "123456ck", (40.778913, 30.427188)),
    ("mike.mitchell.71", "123456ck", (40.748018, 30.353779)),
    ("derya.tas.72", "123456ck", (40.780913, 30.398983)),
    ("berke.young.73", "123456ck", (40.772791, 30.378945)),
    ("defne.mitchell.74", "123456ck", (40.749728, 30.419871)),
    ("clara.lewis.75", "123456ck", (40.711687, 30.361818)),
    ("evren.eren.76", "123456ck", (40.690741, 30.268118)),
    ("ruzgar.scott.77", "123456ck", (40.767631, 30.442292)),
    ("ece.walker.78", "123456ck", (40.791185, 30.358608)),
    ("defne.roberts.79", "123456ck", (40.758226, 30.387173)),
    ("yasemin.gonzalez.80", "123456ck", (40.740345, 30.431216)),
    ("derya.gunduz.81", "123456ck", (40.808225, 30.413828)),
    ("simge.gul.82", "123456ck", (40.687907, 30.351570)),
    ("hugo.tekin.83", "123456ck", (40.718089, 30.393934)),
    ("polat.gonzalez.84", "123456ck", (40.775336, 30.432176)),
    ("berke.kilic.85", "123456ck", (40.750261, 30.357232)),
    ("derya.kose.86", "123456ck", (40.780637, 30.399784)),
    ("simge.roberts.88", "123456ck", (40.772492, 30.378879)),
    ("sam.roberts.89", "123456ck", (40.752821, 30.422449)),
    ("doruk.wright.90", "123456ck", (40.717547, 30.366919)),
    ("ruzgar.lopez.91", "123456ck", (40.689814, 30.270141)),
    ("songul.yavuz.92", "123456ck", (40.763845, 30.441232)),
    ("adam.lopez.93", "123456ck", (40.791223, 30.353235)),
    ("helen.kilic.94", "123456ck", (40.756073, 30.391540)),
    ("rose.lewis.95", "123456ck", (40.743652, 30.434578)),
    ("tanju.aydin.96", "123456ck", (40.811496, 30.413904)),
    ("defne.nelson.97", "123456ck", (40.688640, 30.348917)),
    ("ruzgar.simsek.98", "123456ck", (40.717764, 30.392815)),
    ("lale.korkmaz.99", "123456ck", (40.775574, 30.427635)),
    ("ozge.wright.100", "123456ck", (40.750993, 30.358202)),
]

# Sakarya bölgesindeki popüler noktalar (pickup/dropoff için)
LOCATIONS = [
    (40.7604062, 30.3629614, "Adapazarı Merkez"),
    (40.7750000, 30.3800000, "Serdivan"),
    (40.7450000, 30.3500000, "Arifiye"),
    (40.7680000, 30.3720000, "Mithatpaşa"),
    (40.7520000, 30.3650000, "Yeşiltepe"),
    (40.7830000, 30.3900000, "Erenler"),
    (40.7950000, 30.3550000, "Kurtuluş"),
    (40.7580000, 30.4450000, "Topçular"),
    (40.8080000, 30.4100000, "Camili"),
    (40.6850000, 30.3500000, "Kırca"),
    (40.7200000, 30.3950000, "Hanlı"),
    (40.7780000, 30.4300000, "Köprübaşı"),
    (40.7500000, 30.4200000, "Çukurhamam"),
    (40.6920000, 30.2700000, "Sapanca"),
    (40.7650000, 30.4500000, "Yenikent"),
    (40.7400000, 30.4400000, "Karasu Yolu"),
]

RIDE_INTERVAL = (30, 90)  # başarılı yolculuktan sonra kaç saniye bekle
WAIT_TIMEOUT = 120  # sürücü bulunamazsa kaç saniye sonra vazgeç
# ----------------


def log(prefix: str, msg: str):
    print(f"[{prefix}] {msg}", flush=True)


def http_post(url: str, body: dict, headers: dict = None) -> tuple[bool, int, str]:
    data = json.dumps(body).encode()
    h = {"Content-Type": "application/json"}
    if headers:
        h.update(headers)
    req = urllib.request.Request(url, data=data, headers=h, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return True, resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return False, e.code, e.read().decode(errors="replace")
    except urllib.error.URLError as e:
        return False, 0, str(e)


def http_get(url: str, token: str = None) -> tuple[bool, int, str]:
    h = {"Accept": "application/json"}
    if token:
        h["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, headers=h)
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return True, resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return False, e.code, e.read().decode(errors="replace")
    except urllib.error.URLError as e:
        return False, 0, str(e)


async def login(username: str, password: str) -> str | None:
    """Login ol, access token döndür."""
    url = f"{API_BASE}/auth/login"
    ok, code, body = await asyncio.to_thread(
        http_post, url, {"username": username, "password": password}
    )
    if not ok:
        log(f"Y-{username}", f"Login başarısız: {code}")
        return None
    try:
        data = json.loads(body)
        if data.get("success") or data.get("access_token") or data.get("tokens"):
            tokens = data.get("tokens", data)
            return tokens.get("access_token")
        log(f"Y-{username}", f"Login yanıt beklenmeyen format: {body[:200]}")
        return None
    except Exception as e:
        log(f"Y-{username}", f"Login parse hatası: {e}")
        return None


async def request_ride(
    token: str, pickup: tuple, dropoff: tuple, dropoff_name: str, pickup_name: str
) -> int | None:
    """Taksi çağır, ride_id döndür. user_id JWT'den alınır, body'ye eklenmez."""
    url = f"{API_BASE}/ride/request"
    body = {
        "pickup_lat": pickup[0],
        "pickup_lon": pickup[1],
        "pickup_address": pickup_name,
        "dropoff_lat": dropoff[0],
        "dropoff_lon": dropoff[1],
        "dropoff_address": dropoff_name,
    }
    headers = {"Authorization": f"Bearer {token}"}
    ok, code, resp_body = await asyncio.to_thread(http_post, url, body, headers)
    if not ok:
        log("BOT", f"requestRide başarısız: {code} {resp_body[:200]}")
        return None
    try:
        data = json.loads(resp_body)
        return data.get("ride_id")
    except Exception:
        return None


async def run_passenger(username: str, password: str, home: tuple, tag: str):
    """Tek yolcu simülasyonu — sonsuz döngü."""
    log(tag, f"Başlatılıyor: {username}")

    # Login
    token = await login(username, password)
    if not token:
        log(tag, f"❌ Login başarısız: {username}")
        return

    log(tag, f"✅ Login: {username}")

    while True:
        # Rastgele pickup ve dropoff seç
        loc_list = LOCATIONS.copy()
        pickup_loc = random.choice(loc_list)
        loc_list.remove(pickup_loc)
        dropoff_loc = random.choice(loc_list)

        log(tag, f"Taksi çağırılıyor: {pickup_loc[2]} → {dropoff_loc[2]}")

        ride_id = await request_ride(
            token,
            (pickup_loc[0], pickup_loc[1]),
            (dropoff_loc[0], dropoff_loc[1]),
            dropoff_loc[2],
            pickup_loc[2],
        )

        if not ride_id:
            log(tag, "❌ Ride isteği başarısız. 30sn sonra tekrar denenecek...")
            await asyncio.sleep(30)
            continue

        log(tag, f"✅ Ride #{ride_id} oluşturuldu, WS bağlanılıyor...")

        # WS bağlan ve bekle — JWT token ile
        ws_url = f"{WS_BASE}/ws/passenger?token={urllib.parse.quote(token)}"
        try:
            async with websockets.connect(ws_url) as ws:
                log(tag, f"WS bağlantısı kuruldu (ride #{ride_id})")

                # Ping timer
                async def ping_loop():
                    while True:
                        await asyncio.sleep(20)
                        try:
                            await ws.send(json.dumps({"type": "ping"}))
                        except Exception:
                            break

                ping_task = asyncio.create_task(ping_loop())
                ride_done = False
                start_time = asyncio.get_event_loop().time()

                async for raw in ws:
                    elapsed = asyncio.get_event_loop().time() - start_time
                    if elapsed > WAIT_TIMEOUT:
                        log(
                            tag, f"⏱ Timeout ({WAIT_TIMEOUT}sn), yeni ride denenecek..."
                        )
                        break

                    try:
                        msg = json.loads(raw)
                    except Exception:
                        continue

                    mtype = msg.get("type")
                    if mtype == "ride_status_changed":
                        status = msg.get("status")
                        log(tag, f"Ride #{ride_id} durum: {status}")
                        if status == "accepted":
                            log(tag, "🚕 Sürücü kabul etti, bekleniyor...")
                        elif status == "picked_up":
                            log(tag, "🚗 Yolculuk başladı!")
                        elif status == "completed":
                            log(tag, "🎉 Yolculuk tamamlandı!")
                            ride_done = True
                            break
                        elif status in ("cancelled", "no_driver"):
                            log(tag, f"⚠️ Ride bitti: {status}")
                            break
                    elif mtype == "driver_location":
                        lat = msg.get("lat", 0)
                        lon = msg.get("lon", 0)
                        log(tag, f"📍 Sürücü konumu: ({lat:.4f}, {lon:.4f})")

                ping_task.cancel()

                wait_secs = random.randint(*RIDE_INTERVAL)
                if ride_done:
                    log(tag, f"✅ Tamamlandı. {wait_secs}sn sonra yeni ride...")
                else:
                    log(
                        tag, f"Ride bitti/zaman aşımı. {wait_secs}sn sonra yeni ride..."
                    )
                await asyncio.sleep(wait_secs)

        except websockets.exceptions.ConnectionClosedError as e:
            log(tag, f"WS bağlantısı kesildi: {e}. 10sn sonra tekrar...")
            await asyncio.sleep(10)
        except Exception as e:
            log(tag, f"Beklenmedik hata: {e}. 15sn sonra tekrar...")
            await asyncio.sleep(15)


async def main():
    tasks = []
    for i, (username, password, home) in enumerate(PASSENGERS):
        tag = f"Y{i + 1}"
        await asyncio.sleep(i * 5)
        tasks.append(asyncio.create_task(run_passenger(username, password, home, tag)))
    await asyncio.gather(*tasks)


if __name__ == "__main__":

    def handle_exit(sig, frame):
        print("[BOT] Durduruluyor...")
        sys.exit(0)

    signal.signal(signal.SIGINT, handle_exit)
    signal.signal(signal.SIGTERM, handle_exit)

    asyncio.run(main())
