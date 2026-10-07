// Native integration: register observation must preserve PCM and seek output.
#include "../../native/libvgm-kog/kog_libvgm.cpp"
#include <cassert>

static void put32(std::vector<uint8_t>& bytes, size_t offset, uint32_t value) {
    for(unsigned i=0;i<4;++i) bytes[offset+i] = value >> (8*i);
}
int main() {
    size_t checked_cores = 0;
    for(auto declaration = sndEmu_Devices; *declaration; ++declaration) {
        for(auto core = (*declaration)->cores; *core; ++core) {
            KogVgmInspection::Device probe;
            size_t n = 0;
            while((*core)->rwFuncs[n].funcPtr) {
                assert(n < 64);
                probe.functions[n] = (*core)->rwFuncs[n];
                ++n;
            }
            probe.wrap_all(std::make_index_sequence<64>{});
            ++checked_cores;
        }
    }
    std::printf("Validated inspection forwarding for %zu sound cores\n", checked_cores);
    std::vector<uint8_t> file(0x100);
    std::memcpy(file.data(),"Vgm ",4);
    put32(file,8,0x151); put32(file,12,3579545); put32(file,24,44100);
    put32(file,0x2c,7670454); put32(file,0x34,0x100-0x34);
    put32(file,0x74,1789773);
    for(auto pair : {std::pair{0,244}, {1,1}, {7,62}, {8,15}, {9,0}, {10,0}}) {
        file.push_back(0xa0); file.push_back(pair.first); file.push_back(pair.second);
    }
    for(auto value:{0x80,0x10,0x90,0xbf,0xdf,0xff}) { file.push_back(0x50); file.push_back(value); }
    auto fm=[&](int a,int v){file.push_back(0x52); file.push_back(a);file.push_back(v);};
    for(int op=0;op<4;++op){fm(0x30+op*4,1);fm(0x40+op*4,0);fm(0x50+op*4,31);fm(0x80+op*4,15);}
    fm(0xb0,7);fm(0xb4,0xc0);fm(0xa4,0x22);fm(0xa0,0x69);fm(0x28,0xf0);
    for(unsigned i=0;i<60;++i)file.push_back(0x62);
    fm(0x28,0);file.insert(file.end(),{0x50,0x9f,0x66});put32(file,4,file.size()-4);
    char error[512]{};
    auto create=[&](){auto*p=kog_libvgm_create(file.data(),file.size(),nullptr,0,44100,1,0,0,error,sizeof(error)); if(!p)std::fprintf(stderr,"%s\n",error);assert(p);return std::unique_ptr<kog_libvgm>(p);};
    auto observed=create();
    auto plain=std::make_unique<kog_libvgm>();
    assert(plain->initialize(file.data(),file.size(),nullptr,0,44100,1,0,0,false));
    assert(observed->inspection_devices.size()==3);
    std::vector<float>a(2048*2),b(a.size());
    bool heard=false;
    for(int pass=0;pass<2;++pass) {
        if(pass){assert(!kog_libvgm_seek(observed.get(),22050));assert(!kog_libvgm_seek(plain.get(),22050));}
        for(int block=0;block<10;++block) {
            size_t n=kog_libvgm_render(observed.get(),a.data(),2048),m=kog_libvgm_render(plain.get(),b.data(),2048);
            assert(n==m);if(!std::equal(a.begin(),a.begin()+n*2,b.begin())) { size_t i=0;while(i<n*2&&a[i]==b[i])++i;std::fprintf(stderr,"pass=%d block=%d first=%zu captured=%f baseline=%f\n",pass,block,i,a[i],b[i]); std::abort(); }
            heard |= std::any_of(a.begin(),a.begin()+n*2,[](float x){return std::fabs(x)>0.00001;});
            KogVoice voices[64]{};size_t count=kog_libvgm_inspect(observed.get(),voices,64); assert(count==13);
            assert(voices[0].active);const float expected=kog_frequency_key(3579545.0/(32*256));
            assert(std::fabs(voices[0].key-expected)<0.001);assert(!voices[1].active);
            assert(voices[4].active);assert(voices[4].id==256);
            assert(voices[10].active);
            assert(std::fabs(voices[10].key-kog_frequency_key(1789773.0/(16*500)))<0.001);
        }
    }
    assert(heard);
    std::puts("libvgm inspection: exact PCM, observed pitches, multiple chips, stable IDs, and seek passed");
}
