// Whisper-rs transcription service
import { WhisperAPI } from '../types/whisper';

export interface TranscriptionOptions {
  language?: string;
  enableVAD?: boolean; // Voice Activity Detection
  suppressBlank?: boolean;
  suppressNonSpeech?: boolean;
}

export interface TranscriptionSegment {
  text: string;
  startTime: number;
  endTime: number;
  confidence?: number;
}

export interface TranscriptionResult {
  text: string;
  segments: TranscriptionSegment[];
  processingTime: number;
  modelUsed: string;
}

export class WhisperTranscriptionService {
  private static instance: WhisperTranscriptionService;
  private currentModel: string | null = null;
  
  private constructor() {}
  
  static getInstance(): WhisperTranscriptionService {
    if (!WhisperTranscriptionService.instance) {
      WhisperTranscriptionService.instance = new WhisperTranscriptionService();
    }
    return WhisperTranscriptionService.instance;
  }
  
  async initialize(): Promise<void> {
    try {
      await WhisperAPI.init();
      this.currentModel = await WhisperAPI.getCurrentModel();
      
      if (!this.currentModel) {
        // Try to load a default model if none is loaded
        const models = await WhisperAPI.getAvailableModels();
        const availableModel = models.find(m => m.status === 'Available');
        
        if (availableModel) {
          await WhisperAPI.loadModel(availableModel.name);
          this.currentModel = availableModel.name;
          console.log(`Auto-loaded model: ${availableModel.name}`);
        } else {
          throw new Error('No Whisper models available. Please download a model first.');
        }
      }
    } catch (error) {
      console.error('Failed to initialize Whisper transcription service:', error);
      throw error;
    }
  }
  
  async ensureModelLoaded(): Promise<void> {
    const isLoaded = await WhisperAPI.isModelLoaded();
    if (!isLoaded || !this.currentModel) {
      await this.initialize();
    }
  }
  
  async switchModel(modelName: string): Promise<void> {
    try {
      await WhisperAPI.loadModel(modelName);
      this.currentModel = modelName;
      console.log(`Switched to model: ${modelName}`);
    } catch (error) {
      console.error(`Failed to switch to model ${modelName}:`, error);
      throw error;
    }
  }
  
  async transcribeAudio(
    audioData: Float32Array, 
    options: TranscriptionOptions = {}
  ): Promise<TranscriptionResult> {
    await this.ensureModelLoaded();
    
    const startTime = Date.now();
    
    try {
      // Convert Float32Array to regular array for Tauri
      const audioArray = Array.from(audioData);
      
      // Call whisper-rs for transcription
      const text = await WhisperAPI.transcribeAudio(audioArray);
      
      const processingTime = Date.now() - startTime;
      
      // For now, return basic result - future enhancement can parse segments
      const result: TranscriptionResult = {
        text: text.trim(),
        segments: [{
          text: text.trim(),
          startTime: 0,
          endTime: audioData.length / 16000, // Assume 16kHz sample rate
          confidence: 1.0
        }],
        processingTime,
        modelUsed: this.currentModel || 'unknown'
      };
      
      console.log(`Transcription completed in ${processingTime}ms using ${this.currentModel}`);
      return result;
      
    } catch (error) {
      console.error('Transcription failed:', error);
      throw new Error(`Transcription failed: ${error}`);
    }
  }
  
  async transcribeAudioBuffer(
    buffer: ArrayBuffer,
    sampleRate: number = 16000
  ): Promise<TranscriptionResult> {
    // Convert ArrayBuffer to Float32Array
    // Assume the buffer contains 32-bit float samples
    const audioData = new Float32Array(buffer);
    
    // Resample if needed (basic implementation)
    let processedAudio: Float32Array = audioData;
    if (sampleRate !== 16000) {
      processedAudio = this.resampleAudio(audioData, sampleRate, 16000);
    }
    
    return this.transcribeAudio(processedAudio);
  }
  
  private resampleAudio(input: Float32Array, fromSampleRate: number, toSampleRate: number): Float32Array {
    if (fromSampleRate === toSampleRate) {
      return input;
    }
    
    const ratio = toSampleRate / fromSampleRate;
    const newLength = Math.round(input.length * ratio);
    const result = new Float32Array(newLength);
    
    for (let i = 0; i < newLength; i++) {
      const srcIndex = Math.round(i / ratio);
      if (srcIndex < input.length) {
        result[i] = input[srcIndex];
      }
    }
    
    return result;
  }
  
  getCurrentModel(): string | null {
    return this.currentModel;
  }
  
  async getAvailableModels() {
    return WhisperAPI.getAvailableModels();
  }
  
  async isReady(): Promise<boolean> {
    try {
      return await WhisperAPI.isModelLoaded();
    } catch {
      return false;
    }
  }
}

// Export singleton instance
export const whisperTranscription = WhisperTranscriptionService.getInstance();