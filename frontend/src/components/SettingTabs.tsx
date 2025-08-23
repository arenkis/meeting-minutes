import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { ModelConfig, ModelSettingsModal } from "./ModelSettingsModal"
import { TranscriptModelProps, TranscriptSettings } from "./TranscriptSettings"
import { About } from "./About";

interface SettingTabsProps {
    modelConfig: ModelConfig;
    setModelConfig: (config: ModelConfig | ((prev: ModelConfig) => ModelConfig)) => void;
    onSave: (config: ModelConfig) => void;
    transcriptModelConfig: TranscriptModelProps;
    setTranscriptModelConfig: (config: TranscriptModelProps) => void;
    onSaveTranscript: (config: TranscriptModelProps) => void;
    setSaveSuccess: (success: boolean | null) => void;
    defaultTab?: string;
}

export function SettingTabs({ 
    modelConfig, 
    setModelConfig, 
    onSave,
    transcriptModelConfig,
    setTranscriptModelConfig,
    onSaveTranscript,
    setSaveSuccess,
    defaultTab = "modelSettings"
}: SettingTabsProps) {

    const handleTabChange = () => {
        setSaveSuccess(null); // Reset save success when tab changes
    };

    return (
        <div className="flex flex-col h-full w-full">
            <Tabs defaultValue={defaultTab} className="flex flex-col h-full w-full" onValueChange={handleTabChange}>
                <TabsList className="flex-shrink-0">
                    <TabsTrigger value="modelSettings">Model Settings</TabsTrigger>
                    <TabsTrigger value="transcriptSettings">🎙️ Transcript Settings</TabsTrigger>
                    <TabsTrigger value="about">About</TabsTrigger>
                </TabsList>
                
                <div className="flex-1 overflow-hidden">
                    <TabsContent value="modelSettings" className="h-full overflow-y-auto p-1">
                        <div className="max-h-full">
                            <ModelSettingsModal
                                modelConfig={modelConfig}
                                setModelConfig={setModelConfig}
                                onSave={onSave}
                            />
                        </div>
                    </TabsContent>
                    
                    <TabsContent value="transcriptSettings" className="h-full overflow-y-auto p-1">
                        <div className="max-h-full">
                            <TranscriptSettings
                                transcriptModelConfig={transcriptModelConfig}
                                setTranscriptModelConfig={setTranscriptModelConfig}
                                onSave={onSaveTranscript}
                            />
                        </div>
                    </TabsContent>
                    
                    <TabsContent value="about" className="h-full overflow-y-auto p-1">
                        <div className="max-h-full">
                            <About />
                        </div>
                    </TabsContent>
                </div>
            </Tabs>
        </div>
    )
}


