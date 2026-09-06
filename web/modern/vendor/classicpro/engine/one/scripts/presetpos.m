#include <lib/std.mi>

Function saveFramePos(int no);
Function gotoFramePos(int no);

Global Group mainGroup;
Global Container main;
Global Layout mylayout;

System.onScriptLoaded() {
	mainGroup = getScriptGroup();
	main = System.getContainer("main");
	mylayout = main.getLayout("normal");
}

System.onKeyDown(String key){
	if(key=="f9") gotoFramePos(1);
	if(key=="f10") gotoFramePos(2);
	if(key=="f11") gotoFramePos(3);
	if(key=="f12") gotoFramePos(4);

	if(key=="shift+f9") saveFramePos(1);
	if(key=="shift+f10") saveFramePos(2);
	if(key=="shift+f11") saveFramePos(3);
	if(key=="shift+f12") saveFramePos(4);
}

saveFramePos(int no){
	setPublicInt("cPro.presetpos."+integerToString(no)+".x", getCurAppLeft());
	setPublicInt("cPro.presetpos."+integerToString(no)+".y", getCurAppTop());
	setPublicInt("cPro.presetpos."+integerToString(no)+".w", getCurAppWidth());
	setPublicInt("cPro.presetpos."+integerToString(no)+".h", getCurAppHeight());
}

gotoFramePos(int no){
	mylayout.resize(getPublicInt("cPro.presetpos."+integerToString(no)+".x", getCurAppLeft()),getPublicInt("cPro.presetpos."+integerToString(no)+".y", getCurAppTop()),getPublicInt("cPro.presetpos."+integerToString(no)+".w", getCurAppWidth()),getPublicInt("cPro.presetpos."+integerToString(no)+".h", getCurAppHeight()));
}