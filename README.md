# HRM API v2 (Rust)

Segunda versão da API do HRM (Human Relationship Manager), reescrita em Rust utilizando Axum, Tokio e SQLx, mantendo compatibilidade com as rotas, payload JSON, regras de negócio e persistência do banco PostgreSQL da versão original em Rails.

---

## Tecnologias

- Linguagem: Rust (edição 2021)
- Web Framework: Axum 0.7
- Async Runtime: Tokio
- Banco de Dados: PostgreSQL com SQLx
- Autenticação: JWT RS256 com verificação de chave pública RSA (config/keys/public.pem) integrada ao unified_login
- Serialização/JSON: Serde / Serde JSON
- Inteligência Artificial (Smart Input):
  - DeepSeek Chat e Vision (deepseek-v4-flash-vision-exp / deepseek-chat)
  - Google Gemini (Gemini 3.7 / 3.5 / 2.5) com fallback automático

---

## Estrutura do Projeto

```
hrm-api-v2/
├── config/
│   └── keys/
│       └── public.pem         # Chave pública RSA para validação dos tokens JWT
├── src/
│   ├── auth.rs                # Validação de JWT RS256, extração de AuthUser e tenant_id
│   ├── config.rs              # Leitura de variáveis de ambiente com valores padrão
│   ├── db.rs                  # Pool de conexões PostgreSQL (SQLx)
│   ├── error.rs               # Tratamento centralizado de erros e respostas HTTP
│   ├── main.rs                # Configuração de rotas, middleware CORS/Tracing e servidor Axum
│   ├── models/                # Structs de dados e requests/responses
│   │   ├── activity.rs
│   │   ├── category.rs
│   │   ├── company.rs
│   │   ├── contact.rs
│   │   ├── dashboard.rs
│   │   ├── expense.rs
│   │   ├── lead.rs
│   │   ├── leads_file.rs
│   │   ├── pagination.rs
│   │   └── project.rs
│   ├── handlers/              # Handlers HTTP de cada recurso
│   │   ├── activities.rs
│   │   ├── categories.rs
│   │   ├── companies.rs
│   │   ├── contacts.rs
│   │   ├── dashboard.rs
│   │   ├── expenses.rs
│   │   ├── health.rs
│   │   ├── leads.rs
│   │   ├── leads_files.rs
│   │   ├── projects.rs
│   │   ├── smart_input.rs
│   │   └── users.rs
│   └── services/              # Serviços e regras de negócio
│       ├── lead_conversion.rs
│       ├── leads_file_processor.rs
│       ├── storage.rs         # Compatibilidade com ActiveStorage do PostgreSQL / Disco
│       └── smart_input/
│           ├── builder.rs
│           ├── deepseek.rs
│           ├── extractor.rs
│           └── gemini.rs
├── .env                       # Variáveis de ambiente locais
├── .env.example               # Template de variáveis de ambiente
├── Cargo.toml
└── README.md
```

---

## Como Executar na Máquina Local

### 1. Pré-requisitos
- Rust e Cargo instalados
- PostgreSQL em execução (ex: container Docker na porta 5432)

### 2. Configurar Variáveis de Ambiente
O arquivo `.env` já vem configurado por padrão:
```env
DATABASE_URL=postgres://development:development@localhost:5432/hrm_development
PORT=3000
HOST=0.0.0.0
UNIFIED_LOGIN_URL=http://localhost:3001
GEMINI_API_KEY=
DEEPSEEK_API_KEY=
PUBLIC_KEY_PATH=config/keys/public.pem
STORAGE_DIR=../hrm-api/storage
RUST_LOG=info,hrm_api_v2=debug
```

### 3. Rodar a Aplicação
```bash
cargo run
```

---

## Endpoints Disponíveis

| Método | Endpoint | Descrição |
|---|---|---|
| GET | / | Retorna `{"status": "online"}` |
| GET | /up | Health check da API |
| GET | /dashboard/stats | Estatísticas agregadas (empresas, contatos, leads, despesas, gráficos mensais) |
| GET | /companies | Listagem paginada de Empresas (filtros por nome e categoria) |
| POST | /companies | Criação de Empresa |
| GET | /companies/:id | Detalhes da Empresa |
| PATCH/PUT | /companies/:id | Atualização da Empresa |
| GET | /contacts | Listagem paginada de Contatos (filtros por nome e e-mail) |
| POST | /contacts | Criação de Contato |
| GET | /contacts/:id | Detalhes do Contato |
| PATCH/PUT | /contacts/:id | Atualização do Contato |
| GET | /categories | Listagem de Categorias (do tenant e globais) |
| POST | /categories | Criação de Categoria (suporte a categorias globais para administradores) |
| GET | /categories/:id | Detalhes da Categoria |
| PATCH/PUT | /categories/:id | Atualização da Categoria (com sincronização nas empresas associadas) |
| GET | /users | Proxy de Usuários para o unified_login |
| GET | /leads | Listagem paginada de Leads |
| POST | /leads | Criação de Lead |
| GET | /leads/:id | Detalhes do Lead |
| PATCH/PUT | /leads/:id | Atualização do Lead |
| DELETE | /leads/:id | Remoção do Lead |
| POST | /leads/:id/convert | Conversão de Lead em Empresa e Contato |
| GET | /leads_files | Listagem de Arquivos de Leads importados |
| POST | /leads_files | Upload multipart de arquivo CSV de Leads para processamento em background |
| GET | /leads_files/:id | Status detalhado do processamento e linhas com erro |
| GET | /leads_files/:id/download | Download do arquivo CSV original |
| GET | /projects | Listagem paginada de Projetos |
| POST | /projects | Criação de Projeto |
| GET | /projects/:id | Detalhes do Projeto com Atividades e Despesas |
| PATCH/PUT | /projects/:id | Atualização de Projeto |
| POST | /projects/:project_id/activities | Criação de Atividade associada a um Projeto e múltiplos Contatos |
| PATCH/PUT | /activities/:id | Atualização de Atividade |
| GET | /expenses | Listagem paginada de Despesas |
| POST | /expenses | Criação de Despesa (com suporte a anexo de comprovante em Base64 ou Multipart) |
| GET | /expenses/:id | Detalhes da Despesa com informações do Comprovante |
| PATCH/PUT | /expenses/:id | Atualização de Despesa e comprovante |
| GET | /expenses/:id/receipt | Visualização/Download do comprovante da despesa |
| POST | /smart_input/analyze | Análise por IA (OCR e extração estruturada de áudio, texto ou imagem) |
| POST | /smart_input/commit | Persistência transacional em lote das entidades extraídas pelo Smart Input |
