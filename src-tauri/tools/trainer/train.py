import torch
import os
import sys
from datasets import load_dataset
from peft import LoraConfig, get_peft_model, prepare_model_for_kbit_training
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    BitsAndBytesConfig,
    TrainingArguments,
)
from trl import SFTTrainer

# --- CONFIGURATION PAR DÉFAUT ---
# Vous pourrez changer ces valeurs ou les passer en arguments plus tard
MODEL_ID = "mistralai/Mistral-7B-Instruct-v0.2" 
NEW_MODEL_NAME = "genaptitude-lora-adapter"
DATASET_FILE = "dataset.jsonl" 

def train():
    print(f"🚀 Initialisation de l'entraînement QLoRA sur {MODEL_ID}")

    # 1. Vérification du Dataset
    if not os.path.exists(DATASET_FILE):
        print(f"❌ Erreur: Le fichier de données '{DATASET_FILE}' est introuvable.")
        print("   Veuillez d'abord exporter les données depuis GenAptitude.")
        sys.exit(1)

    # 2. Configuration QLoRA (4-bit Quantization)
    bnb_config = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.float16,
    )

    # 3. Chargement du Modèle de base
    print("⏳ Chargement du modèle (peut prendre du temps)...")
    try:
        model = AutoModelForCausalLM.from_pretrained(
            MODEL_ID,
            quantization_config=bnb_config,
            device_map="auto" # Utilise le GPU si disponible
        )
        model.config.use_cache = False
        model.config.pretraining_tp = 1
    except Exception as e:
        print(f"❌ Erreur chargement modèle: {e}")
        sys.exit(1)

    tokenizer = AutoTokenizer.from_pretrained(MODEL_ID, trust_remote_code=True)
    tokenizer.pad_token = tokenizer.eos_token
    tokenizer.padding_side = "right"

    # 4. Configuration LoRA (Low-Rank Adaptation)
    peft_config = LoraConfig(
        lora_alpha=16,
        lora_dropout=0.1,
        r=64, # Rank: plus élevé = plus de paramètres apprenables (max 128 recommandé)
        bias="none",
        task_type="CAUSAL_LM",
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj"]
    )

    model = prepare_model_for_kbit_training(model)
    model = get_peft_model(model, peft_config)

    # 5. Chargement des Données
    print(f"📂 Chargement du dataset: {DATASET_FILE}")
    dataset = load_dataset("json", data_files=DATASET_FILE, split="train")

    # 6. Paramètres d'entraînement
    training_args = TrainingArguments(
        output_dir="./results",
        num_train_epochs=1,          # Nombre de passes sur les données
        per_device_train_batch_size=4,
        gradient_accumulation_steps=1,
        optim="paged_adamw_32bit",   # Optimiseur économe en mémoire
        save_steps=50,
        logging_steps=10,
        learning_rate=2e-4,
        weight_decay=0.001,
        fp16=True,
        bf16=False,
        max_grad_norm=0.3,
        max_steps=-1,
        warmup_ratio=0.03,
        group_by_length=True,
        lr_scheduler_type="constant",
    )

    # 7. Lancement du Trainer (Supervised Fine-Tuning)
    trainer = SFTTrainer(
        model=model,
        train_dataset=dataset,
        peft_config=peft_config,
        dataset_text_field="text", # Le champ JSON contenant le prompt formaté
        max_seq_length=None,
        tokenizer=tokenizer,
        args=training_args,
        packing=False,
    )

    print("🔥 Démarrage du Fine-Tuning...")
    trainer.train()

    # 8. Sauvegarde
    print(f"💾 Sauvegarde de l'adaptateur dans './{NEW_MODEL_NAME}'...")
    trainer.model.save_pretrained(NEW_MODEL_NAME)
    print("✅ Entraînement terminé avec succès !")

if __name__ == "__main__":
    train()