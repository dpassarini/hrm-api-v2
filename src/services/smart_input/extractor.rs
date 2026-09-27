use chrono::Utc;
use serde_json::Value;

use crate::config::AppConfig;
use super::{deepseek::DeepseekProvider, gemini::GeminiProvider};

pub struct ExtractorService;

impl ExtractorService {
    pub async fn extract(
        text: Option<&str>,
        image: Option<(&str, &str)>, // (mime_type, base64_data)
        preferred_provider: Option<&str>,
    ) -> Option<Value> {
        let config = AppConfig::get();
        let prompt = Self::build_prompt(text.unwrap_or(""));

        let deepseek_key = config.deepseek_api_key.as_deref();
        let gemini_key = config.gemini_api_key.as_deref();

        if deepseek_key.is_none() && gemini_key.is_none() {
            tracing::error!("No API keys configured (GEMINI_API_KEY or DEEPSEEK_API_KEY)");
            return None;
        }

        let provider_pref = preferred_provider.map(|p| p.to_lowercase());

        if provider_pref.as_deref() == Some("deepseek") {
            if let Some(key) = deepseek_key {
                if let Some(res) = DeepseekProvider::extract(&prompt, key, image).await {
                    return Some(res);
                }
            }
            if let Some(key) = gemini_key {
                if let Some(res) = GeminiProvider::extract(&prompt, key, image).await {
                    return Some(res);
                }
            }
        } else if provider_pref.as_deref() == Some("gemini") {
            if let Some(key) = gemini_key {
                if let Some(res) = GeminiProvider::extract(&prompt, key, image).await {
                    return Some(res);
                }
            }
            if let Some(key) = deepseek_key {
                if let Some(res) = DeepseekProvider::extract(&prompt, key, image).await {
                    return Some(res);
                }
            }
        } else {
            // Auto mode
            if let Some(key) = deepseek_key {
                if let Some(res) = DeepseekProvider::extract(&prompt, key, image).await {
                    return Some(res);
                }
            }
            if let Some(key) = gemini_key {
                if let Some(res) = GeminiProvider::extract(&prompt, key, image).await {
                    return Some(res);
                }
            }
        }

        tracing::error!("SmartInput: Todos os provedores de IA falharam na extração.");
        None
    }

    fn build_prompt(input_text: &str) -> String {
        let today_str = Utc::now().format("%Y-%m-%d").to_string();

        format!(
            r#"Você é um assistente de extração de dados e OCR altamente inteligente para o sistema HRM (Human Relationship Manager) e CRM de hospitalidade/turismo/negócios.
Data de referência de hoje: {}.

Abaixo você receberá um texto, transcrição de áudio, anotação informal, assinatura de e-mail e/ou uma imagem de comprovante/recibo/cupom fiscal/cartão de visita.

Sua missão é identificar todas as entidades de negócio e estruturá-las rigorosamente no formato JSON abaixo.

REGRAS DE EXTRAÇÃO:
1. "companies" (Empresas / Estabelecimentos):
   - 'name' é obrigatório (ex: nome da agência parceira ou estabelecimento onde ocorreu o gasto/reunião).
   - 'categories': Array com 1 ou mais categorias inferidas (ex: ["Agência de Viagens", "Restaurante", "Hotelaria", "Transporte", "Corporativo"]).
2. "contacts" (Contatos / Pessoas):
   - 'name' é obrigatório.
   - Extraia 'email', 'phone' / whatsapp, 'job_title' (cargo) e 'company_name' (empresa vinculada).
3. "projects" (Projetos / Campanhas):
   - 'name' é obrigatório. 'status': "planned", "in_progress", "completed" ou "cancelled" (padrão: "planned").
4. "activities" (Atividades / Reuniões / Treinamentos / Sales Calls):
   - 'title' é obrigatório (ex: "Almoço de relacionamento com Agência X", "Treinamento produto Y").
   - 'status': "pending", "completed" ou "cancelled" (padrão: "completed" se for algo que já ocorreu, ou "pending" se for agendamento futuro).
   - 'due_date': Data do evento no formato YYYY-MM-DD (se identificado).
5. "expenses" (Despesas / Recibos / Comprovantes):
   - 'amount': Número float com o valor total da despesa (ex: 185.50).
   - 'description': Descrição clara do gasto (ex: "Almoço com diretoria da Prime Travel", "Uber para o Hotel FASANO", "Cupom fiscal Restaurante Z").
   - 'date': Data em que o gasto ocorreu (YYYY-MM-DD). Se for um recibo, extraia a data impressa; se não houver data explícita, use {}.
   - 'category': Categoria da despesa (ex: "Alimentação", "Transporte", "Hospedagem", "Eventos", "Outros").
   - 'establishment': Nome do estabelecimento / fornecedor emissor do comprovante.

Responda APENAS com o JSON puro, sem blocos Markdown (sem ```json e sem ```).

Formato JSON:
{{
  "companies": [
    {{ "name": "Nome da Empresa", "categories": ["Categoria"] }}
  ],
  "contacts": [
    {{ "name": "Nome", "email": "email@exemplo.com", "phone": "(11) 99999-9999", "job_title": "Diretor", "company_name": "Nome da Empresa" }}
  ],
  "projects": [
    {{ "name": "Nome do Projeto", "status": "planned" }}
  ],
  "activities": [
    {{ "title": "Reunião Comercial", "status": "completed", "due_date": "{}", "project_name": "Projeto Opcional" }}
  ],
  "expenses": [
    {{ "amount": 150.00, "description": "Descrição do gasto", "date": "{}", "category": "Alimentação", "establishment": "Restaurante X", "project_name": "Projeto Opcional" }}
  ]
}}

Texto/Dados de entrada para análise:
{}"#,
            today_str, today_str, today_str, today_str, input_text
        )
    }
}
