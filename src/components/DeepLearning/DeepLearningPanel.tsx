// FICHIER : src/components/DeepLearning/DeepLearningPanel.tsx
import React, { useState } from 'react';
import { deepLearningService } from '../../services/deepLearningService';

export const DeepLearningPanel: React.FC = () => {
  // --- État Local ---
  const [logs, setLogs] = useState<string[]>([]);
  const [lossHistory, setLossHistory] = useState<number[]>([]);

  // Config Modèle
  const [inputDim, setInputDim] = useState(5);
  const [hiddenDim, setHiddenDim] = useState(10);
  const [outputDim, setOutputDim] = useState(2);

  // Paramètres Entraînement
  const [epochs, setEpochs] = useState(50);
  const [learningRate, setLearningRate] = useState(0.01); // Juste pour l'affichage (le back est en dur pour l'instant)

  // État UI
  const [isTraining, setIsTraining] = useState(false);
  const [modelReady, setModelReady] = useState(false);

  // --- Helpers ---
  const log = (msg: string) =>
    setLogs((prev) => [`[${new Date().toLocaleTimeString()}] ${msg}`, ...prev]);

  // --- Actions ---

  const handleInit = async () => {
    try {
      const res = await deepLearningService.initModel({ inputDim, hiddenDim, outputDim });
      log(`✅ ${res}`);
      setLossHistory([]);
      setModelReady(true);
    } catch (e) {
      log(`❌ Erreur Init: ${e}`);
    }
  };

  const handleTrainLoop = async () => {
    if (isTraining) return;
    setIsTraining(true);
    log(`🚀 Démarrage de l'entraînement (${epochs} itérations)...`);

    try {
      // Simulation : On apprend à détecter si la somme > inputDim / 2
      for (let i = 0; i < epochs; i++) {
        const input = Array.from({ length: inputDim }, () => Math.random());
        const sum = input.reduce((a, b) => a + b, 0);
        const target = sum > inputDim / 2 ? 1 : 0;

        const loss = await deepLearningService.trainStep(input, target);

        setLossHistory((prev) => [...prev.slice(-99), loss]); // Garde les 100 derniers

        // Rafraichissement UI tous les 5 pas pour ne pas lagger
        if (i % 5 === 0) await new Promise((r) => setTimeout(r, 5));
      }
      log('✅ Entraînement terminé.');
    } catch (e) {
      log(`❌ Erreur Train: ${e}`);
    } finally {
      setIsTraining(false);
    }
  };

  const handlePredict = async () => {
    try {
      const inputHigh = Array.from({ length: inputDim }, () => 0.9);
      const resHigh = await deepLearningService.predict(inputHigh);

      const inputLow = Array.from({ length: inputDim }, () => 0.1);
      const resLow = await deepLearningService.predict(inputLow);

      log(`🔮 Input [High] -> ${resHigh.map((n) => n.toFixed(3))}`);
      log(`🔮 Input [Low]  -> ${resLow.map((n) => n.toFixed(3))}`);
    } catch (e) {
      log(`❌ Erreur Predict: ${e}`);
    }
  };

  const handleSave = async () => {
    try {
      const path = 'model_dump.safetensors';
      const res = await deepLearningService.saveModel(path);
      log(`💾 ${res}`);
    } catch (e) {
      log(`❌ Erreur Save: ${e}`);
    }
  };

  // --- RENDU : Sexy Slider Helper ---
  const renderSlider = (
    label: string,
    value: number,
    setValue: (val: number) => void,
    min: number,
    max: number,
    step: number,
  ) => {
    const percentage = ((value - min) / (max - min)) * 100;

    return (
      <div
        style={{
          backgroundColor: 'rgba(255,255,255,0.03)',
          border: '1px solid var(--border-color)',
          borderRadius: '10px',
          padding: '12px',
          marginBottom: '12px',
          boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
        }}
      >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            marginBottom: '8px',
            fontSize: '13px',
          }}
        >
          <span style={{ color: 'var(--text-muted)', fontWeight: 500 }}>{label}</span>
          <span
            style={{
              color: '#fff',
              fontWeight: 'bold',
              background: 'var(--color-accent)', // Bleu/Accent du thème
              padding: '2px 8px',
              borderRadius: '4px',
              fontSize: '11px',
            }}
          >
            {value}
          </span>
        </div>

        <input
          type="range"
          className="sexy-range" // CLASSE GLOBALE REQUISE DANS globals.css
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => setValue(+e.target.value)}
          style={{
            width: '100%',
            height: '4px',
            borderRadius: '2px',
            background: `linear-gradient(90deg, var(--color-accent) ${percentage}%, #444 ${percentage}%)`,
            outline: 'none',
            cursor: 'pointer',
          }}
          disabled={isTraining}
        />

        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            marginTop: '4px',
            fontSize: '10px',
            color: '#666',
          }}
        >
          <span>{min}</span>
          <span>{max}</span>
        </div>
      </div>
    );
  };

  // --- RENDU : Styles ---
  const styles: Record<string, React.CSSProperties> = {
    container: {
      padding: '20px',
      backgroundColor: 'var(--bg-app)',
      color: 'var(--text-main)',
      height: '100%',
      display: 'flex',
      flexDirection: 'column',
    },
    header: {
      marginBottom: '20px',
      borderBottom: '1px solid var(--border-color)',
      paddingBottom: '15px',
    },
    grid: {
      display: 'grid',
      gridTemplateColumns: '320px 1fr',
      gap: '20px',
      flex: 1,
      minHeight: 0,
      overflow: 'hidden',
    },
    leftPanel: {
      backgroundColor: 'var(--bg-panel)',
      padding: '20px',
      borderRadius: '12px',
      border: '1px solid var(--border-color)',
      overflowY: 'auto',
      display: 'flex',
      flexDirection: 'column',
    },
    rightPanel: {
      backgroundColor: 'var(--bg-panel)',
      padding: '20px',
      borderRadius: '12px',
      border: '1px solid var(--border-color)',
      display: 'flex',
      flexDirection: 'column',
      overflowY: 'hidden', // Géré par les sous-conteneurs
    },
    btnMain: {
      width: '100%',
      padding: '14px',
      marginBottom: '10px',
      background: 'var(--color-accent)',
      color: '#fff',
      border: 'none',
      borderRadius: '8px',
      cursor: 'pointer',
      fontWeight: 'bold',
      fontSize: '14px',
      boxShadow: '0 4px 10px rgba(0,0,0,0.3)',
      transition: 'all 0.2s',
      opacity: isTraining ? 0.7 : 1,
    },
    btnSec: {
      flex: 1,
      padding: '10px',
      background: '#444',
      color: '#fff',
      border: 'none',
      borderRadius: '8px',
      cursor: 'pointer',
      fontWeight: 500,
      fontSize: '12px',
    },
    chartContainer: {
      flex: 1,
      minHeight: '200px',
      background: 'rgba(0,0,0,0.2)',
      borderRadius: '8px',
      border: '1px solid #333',
      marginBottom: '20px',
      display: 'flex',
      alignItems: 'flex-end',
      padding: '10px',
      gap: '2px',
      overflow: 'hidden',
    },
    logsContainer: {
      height: '200px',
      background: '#111',
      borderRadius: '8px',
      border: '1px solid #333',
      padding: '10px',
      overflowY: 'auto',
      fontFamily: 'monospace',
      fontSize: '12px',
      color: '#aaa',
    },
  };

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <h2 style={{ color: 'var(--color-accent)', margin: 0 }}>Deep Learning Playground</h2>
        <p style={{ fontSize: '12px', opacity: 0.7, marginTop: '5px' }}>
          Moteur neuronal Candle (Rust) • RNN / LSTM
        </p>
      </header>

      <div style={styles.grid}>
        {/* --- PANNEAU GAUCHE : CONFIG --- */}
        <div style={styles.leftPanel}>
          <h4
            style={{
              margin: '0 0 15px 0',
              fontSize: '14px',
              borderBottom: '1px solid #333',
              paddingBottom: '10px',
            }}
          >
            Architecture
          </h4>

          {renderSlider('Input Dimension', inputDim, setInputDim, 1, 50, 1)}
          {renderSlider('Hidden Size (Neurones)', hiddenDim, setHiddenDim, 4, 128, 4)}
          {renderSlider('Output Classes', outputDim, setOutputDim, 1, 10, 1)}

          <button
            style={{
              ...styles.btnMain,
              background: modelReady ? '#4CAF50' : 'var(--color-accent)',
            }}
            onClick={handleInit}
            disabled={isTraining}
          >
            {modelReady ? '🔄 Réinitialiser Modèle' : '⚡ Initialiser Modèle'}
          </button>

          <h4
            style={{
              margin: '20px 0 15px 0',
              fontSize: '14px',
              borderBottom: '1px solid #333',
              paddingBottom: '10px',
            }}
          >
            Hyperparamètres
          </h4>

          {renderSlider('Batch Epochs', epochs, setEpochs, 10, 500, 10)}
          {/* Faux paramètre pour l'exemple visuel, le backend a 0.01 en dur */}
          {renderSlider('Learning Rate (Simu)', learningRate, setLearningRate, 0.001, 0.1, 0.001)}

          <div style={{ marginTop: 'auto' }}>
            <button
              style={{
                ...styles.btnMain,
                background: isTraining ? '#666' : '#FF9800',
                cursor: modelReady ? 'pointer' : 'not-allowed',
              }}
              onClick={handleTrainLoop}
              disabled={!modelReady || isTraining}
            >
              {isTraining ? '⏳ Entraînement en cours...' : "🔥 Lancer l'Entraînement"}
            </button>

            <div style={{ display: 'flex', gap: '10px' }}>
              <button style={styles.btnSec} onClick={handlePredict} disabled={!modelReady}>
                🔮 Tester
              </button>
              <button style={styles.btnSec} onClick={handleSave} disabled={!modelReady}>
                💾 Sauver
              </button>
            </div>
          </div>
        </div>

        {/* --- PANNEAU DROIT : VISUALISATION --- */}
        <div style={styles.rightPanel}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <h4 style={{ margin: '0 0 15px 0', fontSize: '14px' }}>Courbe de Perte (Loss)</h4>
            {lossHistory.length > 0 && (
              <span style={{ fontSize: '12px', color: '#4CAF50' }}>
                Min: {Math.min(...lossHistory).toFixed(4)}
              </span>
            )}
          </div>

          <div style={styles.chartContainer}>
            {lossHistory.length === 0 && (
              <div
                style={{ width: '100%', textAlign: 'center', color: '#555', alignSelf: 'center' }}
              >
                En attente de données...
              </div>
            )}
            {lossHistory.map((val, idx) => {
              const maxVal = Math.max(...lossHistory, 1.0);
              const heightPct = (val / maxVal) * 100;
              // Dégradé de couleur : Rouge (haut) -> Vert (bas)
              const color = val > 0.5 ? '#F44336' : val > 0.1 ? '#FF9800' : '#4CAF50';

              return (
                <div
                  key={idx}
                  style={{
                    flex: 1,
                    background: color,
                    height: `${Math.max(5, heightPct)}%`,
                    minWidth: '4px',
                    borderRadius: '2px 2px 0 0',
                    opacity: 0.8,
                    transition: 'height 0.2s',
                  }}
                  title={`Epoch ${idx}: ${val.toFixed(4)}`}
                />
              );
            })}
          </div>

          <h4 style={{ margin: '0 0 10px 0', fontSize: '14px' }}>Logs Système</h4>
          <div style={styles.logsContainer}>
            {logs.map((l, i) => (
              <div key={i} style={{ borderBottom: '1px solid #222', padding: '2px 0' }}>
                {l}
              </div>
            ))}
            {logs.length === 0 && <span style={{ color: '#444' }}>Aucun log pour le moment.</span>}
          </div>
        </div>
      </div>
    </div>
  );
};
