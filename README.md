# SQL Editor (Rust Edition)

Szybki, ultra-lekki, w pełni natywny i wieloplatformowy edytor SQL oraz klient baz danych przepisany w całości na język **Rust** z wykorzystaniem biblioteki GUI **egui / eframe**.

![SQL Editor](assets/app.png)

---

## ⚡ Główne funkcjonalności

- **Wieloplatformowość (Cross-platform)**: Natywne wsparcie dla systemów macOS (Apple Silicon & Intel), Windows oraz Linux.
- **Wsparcie dla baz danych**:
  - **SQLite** – wbudowane natywnie bez żadnych zewnętrznych zależności (`rusqlite` bundled).
  - **PostgreSQL** – natywny klient Rust (`postgres`).
  - **MySQL / MariaDB** – natywny klient Rust (`mysql`).
  - **Oracle Database, Microsoft SQL Server, Google BigQuery** – pełna obsługa konfiguracji DSN, dialogu połączeń oraz zapytań.
- **Kolorowanie składni SQL (Syntax Highlighting)**: Błyskawiczny lekki silnik tokenizacji regex, z możliwością włączania/wyłączania w preferencjach i automatycznym omijaniem olbrzymich plików powyżej 200 KB.
- **Inteligentne podpowiadanie kodu (SQL Autocomplete)**:
  - Kontekstowe podpowiedzi słów kluczowych SQL, funkcji wbudowanych, tabel oraz kolumn z aktywnego połączenia.
  - Obsługa notacji z kropką: `tabela.kolumna` lub `alias.kolumna`.
  - Wykrywanie kontekstu po klauzulach `FROM`, `JOIN`, `SELECT`, `WHERE` itd.
  - Nawigacja za pomocą `↑` / `↓` oraz zatwierdzanie klawiszem `Enter` lub `Tab`.
- **Wielozakładkowy edytor (Multi-tab Editing)**: Otwieranie, edycja i zapisywanie wielu plików SQL jednocześnie ze wskaźnikiem niezapisanych zmian (`*`).
- **Bezpieczne przechowywanie połączeń (Encrypted Storage)**: Połączenia i hasła są szyfrowane kluczem AES/XOR 32-bajtowym i zapisywane w formacie w 100% zgodnym z wersją Pythonową w katalogu `~/.sql_editor/connections.json`.
- **Obsługa dużych zbiorów danych (Virtual Scrolling)**: Wirtualne renderowanie tabeli wyników bez zamrażania interfejsu użytkownika, obsługujące nawet 50 000 wierszy płynnie przy 60+ FPS.
- **Wyniki dla wielu zapytań (Multi-query Results)**: Wykonywanie wielu zapytań oddzielonych średnikami i przeglądanie każdego wyniku w osobnej karcie.
- **Eksport wyników**: Eksport do formatów:
  - **CSV**
  - **JSON**
  - **XLSX** (Excel)
  - **Markdown**
- **Kopiowanie komórek i wierszy**:
  - Kliknięcie wiersza i przycisk `📋 Kopiuj wiersz`
  - Podwójne kliknięcie w dowolną komórkę kopiuje jej wartość do schowka
  - Menu kontekstowe pod prawym przyciskiem myszy: *Kopiuj wartość komórki*, *Kopiuj wiersz (TSV)*, *Kopiuj wiersz (CSV)*, *Kopiuj z nagłówkami*
- **Wyraźne wartości NULL**: Wartości `NULL` są wyróżniane kursywą i dedykowanym kolorem zamiast pustych komórek.
- **Motywy i personalizacja (7 motywów)**:
  - 7 wbudowanych szablonów: **Dark (Default)**, **Dracula**, **One Dark Pro**, **Monokai**, **Nord**, **Solarized Dark**, **GitHub Light**.
  - Dostosowywanie kolorów: tło edytora, panele boczne, nagłówki, kolor tekstu, akcent przycisków.
  - Podgląd na żywo (Live preview) w oknie Ustawień.
  - Płynne powiększanie czcionki suwakiem lub skrótem `Ctrl+Scroll` / `Cmd+Scroll`.
- **Wielojęzyczny interfejs (i18n)**:
  - **Polski (pl)**
  - **English (en)**
  - **Deutsch (de)**
  - **Español (es)**
- **Sprawdzanie aktualizacji**:
  - Integracja z GitHub Releases API (`Mixo123/SQL-Editor`).
  - Automatyczne powiadomienie o nowej wersji i okno dialogowe z changelogiem oraz przyciskami bezpośredniego pobierania instalatorów dedykowanych dla danego systemu operacyjnego (.dmg/.pkg na macOS, .msi na Windows, .AppImage na Linux).
- **Ochrona przed zapytaniami bez limitu**: Ostrzeżenie przed wykonaniem zapytań `SELECT` bez klauzuli `LIMIT` (konfigurowalne w ustawieniach).

---

## ⌨️ Skróty klawiszowe

| Skrót | Działanie |
|---|---|
| `Ctrl+Enter` / `Cmd+Enter` | Uruchom zapytanie |
| `F5` | Uruchom zapytanie |
| `Ctrl+S` / `Cmd+S` | Zapisz plik |
| `Ctrl+Shift+S` / `Cmd+Shift+S` | Zapisz jako… |
| `Ctrl+O` / `Cmd+O` | Otwórz plik SQL |
| `Ctrl+T` / `Cmd+T` | Nowa karta |
| `Ctrl+W` / `Cmd+W` | Zamknij aktywną kartę |
| `Ctrl+E` / `Cmd+E` | Eksportuj wyniki do CSV |
| `Ctrl+Scroll` / `Cmd+Scroll` | Powiększ / pomniejsz rozmiar czcionki |
| `Podwójny klik` (na komórce) | Kopiuj wartość komórki do schowka |
| `Prawy klik` (na komórce/wierszu) | Menu kopiowania w różnych formatach (TSV, CSV, z nagłówkami) |
| `Prawy klik` (na połączeniu) | Edytuj lub usuń połączenie |

---

## 🚀 Uruchomienie i kompilacja

### Wymagania
- Rust 1.80+ (zalecany najnowszy stable) oraz `cargo`.

### Uruchomienie ze źródeł
```bash
cd SqlEditor
cargo run
```

### Uruchomienie w trybie zoptymalizowanym (Release)
```bash
cd SqlEditor
cargo run --release
```

### Uruchomienie testów
```bash
cargo test
```
Wszystkie testy jednostkowe i integracyjne weryfikujące szyfrowanie, analizę SQL, bazę SQLite, schematy, eksporty, szablony i tłumaczenia przechodzą w 100%.
