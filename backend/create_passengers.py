import requests
import json
import random

ENDPOINT = "https://one.web.tr/api/auth/register"
PASSWORD = "123456ck"

first_names = [
    "Kemal", "Ege", "Alp", "Doruk", "Rüzgar", "Toprak", "Mert", "Arda", "Yaman", "Fırat",
    "Eren", "Kerem", "Polat", "Berke", "Çınar", "Göktuğ", "Utku", "Evren", "Tanju", "Kutay",
    "Defne", "Lale", "Aylin", "Selin", "Ece", "Yasemin", "Derya", "Gizem", "Özge", "Pınar",
    "Burcu", "İpek", "Sevgi", "Sertab", "Nazlı", "Damla", "Simge", "Tuba", "Songül", "Funda",
    "Mike", "Sara", "Max", "Zoe", "Jack", "Rose", "Ben", "Kate", "Sam", "Lily",
    "Ryan", "Clara", "Adam", "Helen", "Mark", "Lucy", "Paul", "Daisy", "Hugo", "Iris"
]

last_names = [
    "Güner", "Eren", "Tekin", "Kılıç", "Köse", "Aksu", "Yalçın", "Şimşek", "Korkmaz", "Taş",
    "Özkan", "Polat", "Durmuş", "Yavuz", "Karadeniz", "Gündüz", "Aydın", "Çalışkan", "Baş", "Gül",
    "Schmidt", "Gonzalez", "Clark", "Lewis", "Walker", "Hall", "Young", "King", "Wright", "Lopez",
    "Hill", "Scott", "Green", "Adams", "Baker", "Nelson", "Carter", "Mitchell", "Roberts", "Turner"
]

used_emails = set()

def make_user(i):
    first = random.choice(first_names)
    last = random.choice(last_names)

    email_base = f"{first.lower()}.{last.lower()}".replace("ç", "c").replace("ğ", "g").replace("ı", "i").replace("ö", "o").replace("ş", "s").replace("ü", "u")

    email = f"{email_base}.{i}@one.web.tr"
    if email in used_emails:
        email = f"{email_base}{i}@one.web.tr"
    used_emails.add(email)

    username = f"{first.lower()}.{last.lower()}.{i}".replace("ç", "c").replace("ğ", "g").replace("ı", "i").replace("ö", "o").replace("ş", "s").replace("ü", "u")

    return {
        "username": username,
        "email": email,
        "password": PASSWORD,
        "first_name": first,
        "last_name": last,
    }

success = 0
fail = 0

for i in range(1, 101):
    user = make_user(i)
    try:
        r = requests.post(ENDPOINT, json=user, timeout=10)
        if r.status_code in (200, 201):
            success += 1
            print(f"[OK] {i:3d} {user['email']} → {r.status_code}")
        else:
            fail += 1
            print(f"[FAIL] {i:3d} {user['email']} → {r.status_code} {r.text[:120]}")
    except Exception as e:
        fail += 1
        print(f"[ERROR] {i:3d} {user['email']} → {e}")

print(f"\nDone: {success} success, {fail} failed")
