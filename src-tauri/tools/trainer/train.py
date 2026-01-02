import torch
import os
import sys

# Désactivation de WANDB
os.environ["WANDB_DISABLED"] = "true"

from datasets import load_dataset
from peft import LoraConfig, prepare_model_for_kbit_training
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    BitsAndBytesConfig,
)
from trl import SFTTrainer, SFTConfig

# --- CONFIGURATION ---
MODEL_ID = "Qwen/Qwen2.5-1.5B-Instruct"
NEW_MODEL_NAME = "raise-qwen-adapter"
DATASET_FILE = "dataset.jsonl" 

def train():
    print(f"🚀 Initialisation de l'entraînement (Mode Compatibilité T4) sur {MODEL_ID}")

    if not os.path.exists(DATASET_FILE):
        print(f"❌ Erreur: Le fichier '{DATASET_FILE}' est introuvable.")
        sys.exit(1)

    # 1. Configuration QLoRA (4-bit)
    # CHANGEMENT CRUCIAL : On calcule en float32. 
    # C'est plus lent (un peu) mais ça marche à 100% sur T4.
    bnb_config = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.float32, 
    )

    # 2. Chargement Modèle
    print("⏳ Chargement du modèle Qwen...")
    model = AutoModelForCausalLM.from_pretrained(
        MODEL_ID,
        quantization_config=bnb_config,
        device_map="auto",
        trust_remote_code=True,
        # On force le modèle à se voir en float32
        torch_dtype=torch.float32 
    )
    
    # Préparation
    model.config.use_cache = False
    model.config.pretraining_tp = 1
    model = prepare_model_for_kbit_training(model)

    # Configuration Tokenizer
    tokenizer = AutoTokenizer.from_pretrained(MODEL_ID, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    tokenizer.padding_side = "right"

    # 3. LoRA Config
    peft_config = LoraConfig(
        lora_alpha=16,
        lora_dropout=0.1,
        r=64,
        bias="none",
        task_type="CAUSAL_LM",
        target_modules=[
            "q_proj", "k_proj", "v_proj", "o_proj", 
            "gate_proj", "up_proj", "down_proj"
        ]
    )

    dataset = load_dataset("json", data_files=DATASET_FILE, split="train")

    # 4. Configuration Entraînement
    training_args = SFTConfig(
        output_dir="./results",
        dataset_text_field="text",
        num_train_epochs=1,
        per_device_train_batch_size=4,
        gradient_accumulation_steps=1,
        optim="paged_adamw_32bit",
        save_steps=50,
        logging_steps=10,
        learning_rate=2e-4,
        weight_decay=0.001,
        
        # --- MODE SECURISÉ T4 ---
        fp16=False,      # On désactive FP16 (Adieu Gradient Scaler buggé)
        bf16=False,      # On désactive BF16
        # ----------------------
        
        report_to="none", 
        max_grad_norm=0.3,
        max_steps=-1,
        warmup_ratio=0.03,
        group_by_length=True,
        lr_scheduler_type="constant",
        packing=False,
    )

    # 5. Trainer
    trainer = SFTTrainer(
        model=model,
        train_dataset=dataset,
        peft_config=peft_config,
        processing_class=tokenizer,
        args=training_args,
    )

    print("🔥 Démarrage du Fine-Tuning Qwen...")
    trainer.train()

    print(f"💾 Sauvegarde dans './{NEW_MODEL_NAME}'...")
    trainer.model.save_pretrained(NEW_MODEL_NAME)
    print("✅ Terminé !")

if __name__ == "__main__":
    train()