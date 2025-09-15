import { useState, useEffect } from 'react';
import { useSidebar } from './Sidebar/SidebarProvider';
import { invoke } from '@tauri-apps/api/core';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Label } from './ui/label';
import { Eye, EyeOff, Lock, Unlock } from 'lucide-react';
import { ModelManager } from './WhisperModelManager';


export interface TranscriptModelProps {
    provider: 'localWhisper' | 'deepgram' | 'elevenLabs' | 'groq' | 'openai';
    model: string;
    apiKey?: string | null;
}

export interface TranscriptSettingsProps {
    transcriptModelConfig: TranscriptModelProps;
    setTranscriptModelConfig: (config: TranscriptModelProps) => void;
    onSave: (config: TranscriptModelProps) => void;
}

export function TranscriptSettings({ transcriptModelConfig, setTranscriptModelConfig, onSave }: TranscriptSettingsProps) {
    const [apiKey, setApiKey] = useState<string | null>(transcriptModelConfig.apiKey || null);
    const [showApiKey, setShowApiKey] = useState<boolean>(false);
    const [isApiKeyLocked, setIsApiKeyLocked] = useState<boolean>(true);
    const [isLockButtonVibrating, setIsLockButtonVibrating] = useState<boolean>(false);
    const [selectedWhisperModel, setSelectedWhisperModel] = useState<string>(transcriptModelConfig.model || 'large-v3');
    const { serverAddress } = useSidebar();

    useEffect(() => {
        if (transcriptModelConfig.provider === 'localWhisper') {
            setApiKey(null);
        }
    }, [transcriptModelConfig.provider]);

    const fetchApiKey = async (provider: string) => {
        try {

            const data = await invoke('api_get_transcript_api_key', { provider }) as string;

            setApiKey(data || '');
        } catch (err) {
            console.error('Error fetching API key:', err);
            setApiKey(null);
        }
    };
    const modelOptions = {
        localWhisper: [selectedWhisperModel],
        deepgram: ['nova-2-phonecall'],
        elevenLabs: ['eleven_multilingual_v2'],
        groq: ['llama-3.3-70b-versatile'],
        openai: ['gpt-4o'],
    };
    const requiresApiKey = transcriptModelConfig.provider === 'deepgram' || transcriptModelConfig.provider === 'elevenLabs' || transcriptModelConfig.provider === 'openai' || transcriptModelConfig.provider === 'groq';
    const isDoneDisabled = requiresApiKey && (!apiKey || (typeof apiKey === 'string' && !apiKey.trim()))

    const handleSave = () => {
        const updatedConfig = { 
            ...transcriptModelConfig, 
            model: transcriptModelConfig.provider === 'localWhisper' ? selectedWhisperModel : transcriptModelConfig.model,
            apiKey: typeof apiKey === 'string' ? apiKey.trim() || null : null 
        };
        setTranscriptModelConfig(updatedConfig);
        onSave(updatedConfig);
    };

    const handleInputClick = () => {
        if (isApiKeyLocked) {
            setIsLockButtonVibrating(true);
            setTimeout(() => setIsLockButtonVibrating(false), 500);
        }
    };
   
    const handleWhisperModelSelect = (modelName: string) => {
        setSelectedWhisperModel(modelName);
        if (transcriptModelConfig.provider === 'localWhisper') {
            setTranscriptModelConfig({
                ...transcriptModelConfig,
                model: modelName
            });
        }
    };
    return (
        <div className='max-h-[calc(100vh-200px)] overflow-y-auto'>
            <div className="flex justify-between items-center mb-4">
                <h3 className="text-lg font-semibold text-gray-900">Transcript Settings</h3>
            </div>
            <div className="space-y-4 pb-6">
                <div>
                    <Label className="block text-sm font-medium text-gray-700 mb-1">
                        Transcript Model
                    </Label>
                    <div className="flex space-x-2 mx-1">
                        <Select
                            value={transcriptModelConfig.provider}
                            onValueChange={(value) => {
                                const provider = value as TranscriptModelProps['provider'];
                                const newModel = provider === 'localWhisper' ? selectedWhisperModel : modelOptions[provider][0];
                                setTranscriptModelConfig({ ...transcriptModelConfig, provider, model: newModel });
                                if (provider !== 'localWhisper') {
                                    fetchApiKey(provider);
                                }
                            }}
                        >
                            <SelectTrigger className='focus:ring-1 focus:ring-blue-500 focus:border-blue-500'>
                                <SelectValue placeholder="Select provider" />
                            </SelectTrigger>
                            <SelectContent>
                                <SelectItem value="localWhisper">🏠 Local Whisper (Recommended)</SelectItem>
                                <SelectItem value="deepgram">☁️ Deepgram (Backup)</SelectItem>
                                <SelectItem value="elevenLabs">☁️ ElevenLabs</SelectItem>
                                <SelectItem value="groq">☁️ Groq</SelectItem>
                                <SelectItem value="openai">☁️ OpenAI</SelectItem>
                            </SelectContent>
                        </Select>

                        {transcriptModelConfig.provider !== 'localWhisper' && (
                            <Select
                                value={transcriptModelConfig.model}
                                onValueChange={(value) => {
                                    const model = value as TranscriptModelProps['model'];
                                    setTranscriptModelConfig({ ...transcriptModelConfig, model });
                                }}
                            >
                                <SelectTrigger className='focus:ring-1 focus:ring-blue-500 focus:border-blue-500'>
                                    <SelectValue placeholder="Select model" />
                                </SelectTrigger>
                                <SelectContent>
                                    {modelOptions[transcriptModelConfig.provider].map((model) => (
                                        <SelectItem key={model} value={model}>{model}</SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        )}

                    </div>
                </div>

                {transcriptModelConfig.provider === 'localWhisper' && (
                    <div className="mt-6">
                        <ModelManager
                            selectedModel={selectedWhisperModel}
                            onModelSelect={handleWhisperModelSelect}
                        />
                    </div>
                )}

                {transcriptModelConfig.provider === 'localWhisper' && (
                    <div className="bg-blue-50 border border-blue-200 rounded-lg p-4 mt-4">
                        <div className="flex items-start space-x-3">
                            <span className="text-blue-600 mt-0.5">💡</span>
                            <div>
                                <h4 className="font-medium text-blue-900">Why Local Whisper?</h4>
                                <ul className="text-sm text-blue-700 mt-1 space-y-1">
                                    <li>• Complete privacy - audio never leaves your device</li>
                                    <li>• No internet required for transcription</li>
                                    <li>• No API costs or rate limits</li>
                                    <li>• Consistent performance regardless of network</li>
                                </ul>
                            </div>
                        </div>
                    </div>
                )}

                {requiresApiKey && (
                    <div>
                        <Label className="block text-sm font-medium text-gray-700 mb-1">
                            API Key
                        </Label>
                        <div className="relative mx-1">
                            <Input
                                type={showApiKey ? "text" : "password"}
                                className={`pr-24 focus:ring-1 focus:ring-blue-500 focus:border-blue-500 ${isApiKeyLocked ? 'bg-gray-100 cursor-not-allowed' : ''
                                    }`}
                                value={apiKey || ''}
                                onChange={(e) => setApiKey(e.target.value)}
                                disabled={isApiKeyLocked}
                                onClick={handleInputClick}
                                placeholder="Enter your API key"
                            />
                            {isApiKeyLocked && (
                                <div
                                    onClick={handleInputClick}
                                    className="absolute inset-0 flex items-center justify-center bg-gray-100 bg-opacity-50 rounded-md cursor-not-allowed"
                                />
                            )}
                            <div className="absolute inset-y-0 right-0 pr-1 flex items-center">
                                <Button
                                    type="button"
                                    variant="ghost"
                                    size="icon"
                                    onClick={() => setIsApiKeyLocked(!isApiKeyLocked)}
                                    className={`transition-colors duration-200 ${isLockButtonVibrating ? 'animate-vibrate text-red-500' : ''
                                        }`}
                                    title={isApiKeyLocked ? "Unlock to edit" : "Lock to prevent editing"}
                                >
                                    {isApiKeyLocked ? <Lock className="h-4 w-4" /> : <Unlock className="h-4 w-4" />}
                                </Button>
                                <Button
                                    type="button"
                                    variant="ghost"
                                    size="icon"
                                    onClick={() => setShowApiKey(!showApiKey)}
                                >
                                    {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                </Button>
                            </div>
                        </div>
                    </div>
                )}
                <div className='mt-6 flex justify-end'>
                    <Button
                        onClick={handleSave}
                        disabled={isDoneDisabled}
                        className={`px-4 py-2 text-sm font-medium text-white rounded-md focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 ${isDoneDisabled
                                ? 'bg-gray-400 cursor-not-allowed'
                                : 'bg-blue-600 hover:bg-blue-700'
                            }`}
                    >
                        Done</Button>
                </div>
            </div>
        </div>
    )
}








