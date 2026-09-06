#include <lib/std.mi>

Global Group mainGroup;
Global Container main;
Global Slider vol_slider;
Global Layer muteWarning;

//MuteButton
Global Togglebutton mute_but, b_muteOverlay;
Global Timer reCheck;

// Mute warning
Function doFade();
Function stopFade();
Function startFade();
Global Boolean direction, createdTooltip, haveOverlay;
Global Timer myTimer;


System.onScriptLoaded() {
	mainGroup = getScriptGroup();
	/*
	vol_slider = mainGroup.findObject("two.playback.volslider");
	mute_but = mainGroup.findObject("two.player.mute");
	muteWarning = mainGroup.findObject("two.player.mute.overlay");
	*/
	vol_slider = mainGroup.findObject(getToken(getParam(),";",0));
	mute_but = mainGroup.findObject(getToken(getParam(),";",1));
	muteWarning = mainGroup.findObject(getToken(getParam(),";",2));
	b_muteOverlay = mainGroup.findObject(getToken(getParam(),";",3));
	myTimer = new Timer;
	myTimer.setDelay(700);
	
	
	if(getToken(getParam(),";",3)!=""){
		Map myMap = new Map;
		myMap.loadMap(getToken(getParam(),";",4));
		if(myMap.getHeight()==30 || myMap.getHeight()==22){
			haveOverlay = true;
			b_muteOverlay.show();
		}
		//overlay2.setRegionFromMap(myMap, 255, 0);
		delete myMap;
	}

}

mainGroup.onSetVisible(boolean onOff){
	if(onOff){
		if(mute_but.getCurCfgVal() != getPrivateInt(getSkinName(), "muted", 0) || !createdTooltip){
			createdTooltip = true;
			mute_but.setActivated(getPrivateInt(getSkinName(), "muted", 0));
			mute_but.onToggle(getPrivateInt(getSkinName(), "muted", 0));
		}
	}
}

mute_but.onSetVisible(boolean onOff){
	if(onOff){
		muteWarning.show();
	}
	else muteWarning.hide();
}

vol_slider.onSetPosition(int newpos){
	if(mute_but.getCurCfgVal()==1 && newpos>0){
		//mute_but.setActivated(false);
		//b_muteOverlay.setActivated(false);
		mute_but.leftClick();
		setPrivateInt(getSkinName(), "muted", 0);
		stopFade();
	}
}
vol_slider.onPostedPosition(int newpos){
	if(mute_but.getCurCfgVal()==1 && newpos>0){
		//mute_but.setActivated(false);
		//b_muteOverlay.setActivated(false);
		mute_but.leftClick();
		setPrivateInt(getSkinName(), "muted", 0);
		stopFade();
	}
}

mute_but.onToggle(Boolean onoff){
	if(mute_but.getCurCfgVal()==0){
		if(getVolume()==0 && getPrivateInt(getSkinName(), "muted", 0)) setVolume(getPrivateInt(getSkinName(), "saveVol", 100));
		mute_but.setXmlParam("tooltip", "Mute Volume");
		stopFade();
		if(haveOverlay) b_muteOverlay.setActivated(false);
	}
	else{
		if(getVolume()!=0) setPrivateInt(getSkinName(), "saveVol", getVolume());
		setVolume(0);
		mute_but.setXmlParam("tooltip", "Turn Volume On");
		startFade();
		if(haveOverlay) b_muteOverlay.setActivated(true);
	}
	setPrivateInt(getSkinName(), "muted", mute_but.getCurCfgVal());
}



//Mute warning
myTimer.onTimer(){
	doFade();
}
doFade(){
	if(direction){
		muteWarning.setTargetA(0);
	}
	else{
		muteWarning.setTargetA(253);
	}
	muteWarning.setTargetSpeed(0.6);
	muteWarning.gotoTarget();
	direction=!direction;
}
startFade(){
	direction=false;
	doFade();
	myTimer.start();
	muteWarning.show();
}
stopFade(){
	myTimer.stop();
	muteWarning.cancelTarget();
	muteWarning.setAlpha(0);
	muteWarning.hide();
}