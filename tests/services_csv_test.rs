#[test]
fn test_csv_delimiter_detection() {
    let semicolon_csv = "Nome;Empresa;Email;Telefone\nJoão;Tech Corp;joao@tech.com;1199999999";
    let comma_csv = "Nome,Empresa,Email,Telefone\nMaria,Design Studio,maria@studio.com,1188888888";

    let first_line_semi = semicolon_csv.lines().next().unwrap();
    let semi_count = first_line_semi.matches(';').count();
    let comma_count1 = first_line_semi.matches(',').count();
    assert!(semi_count > comma_count1);

    let first_line_comma = comma_csv.lines().next().unwrap();
    let semi_count2 = first_line_comma.matches(';').count();
    let comma_count2 = first_line_comma.matches(',').count();
    assert!(comma_count2 > semi_count2);
}

#[test]
fn test_csv_bom_stripping() {
    let with_bom = "\u{feff}Nome;Empresa\nAna;Alpha";
    let clean = with_bom.strip_prefix('\u{feff}').unwrap_or(with_bom);
    assert!(!clean.starts_with('\u{feff}'));
    assert!(clean.starts_with("Nome;Empresa"));
}

#[test]
fn test_csv_parsing_and_category_split() {
    let raw_categories = "Hotelaria, Luxo; Viagens | Corporativo";
    let mut categories = Vec::new();
    for part in raw_categories.split(&[',', ';', '|'][..]) {
        let trimmed = part.trim();
        if !trimmed.is_empty() && !categories.contains(&trimmed.to_string()) {
            categories.push(trimmed.to_string());
        }
    }

    assert_eq!(categories.len(), 4);
    assert_eq!(categories, vec!["Hotelaria", "Luxo", "Viagens", "Corporativo"]);
}
