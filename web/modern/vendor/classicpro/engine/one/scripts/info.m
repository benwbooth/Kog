#include <lib/std.mi>
#define fadeTime 0.3

Function updateInfo(String showThis);
Function showNews(boolean show);
Function cancelStuff();

Global Group mainGroup;
Global Text info_news, xlat_vol;
Global GuiObject info_songticker;
Global Timer fade, goBack;
Global Slider sl_volume, sl_seeker;
Global Button prev, next, open;
Global Boolean isShort, busyWithSeek, cancelNext;
Global Togglebutton shufBut, repBut, muteBut;

System.onScriptLoaded(){
	mainGroup = getScriptGroup();
	info_songticker = mainGroup.findObject("infosongticker");
	info_news = mainGroup.findObject("infodisplay");
	sl_volume = mainGroup.findObject("Volume");
	sl_seeker = mainGroup.findObject("seeker");
	xlat_vol = mainGroup.findObject("xlat.volume");
	prev = mainGroup.findObject("previous.track");
	next = mainGroup.findObject("next.track");
	open = mainGroup.findObject("open.tracks");
	shufBut = mainGroup.findObject("shuffle");
	repBut = mainGroup.findObject("repeat");
	muteBut = mainGroup.findObject("mute");

	fade = new Timer;
	fade.setDelay(300);
	goBack = new Timer;
	busyWithSeek=false;
	cancelNext=false;
}

System.onscriptunloading(){
	delete fade;
	delete goBack;
}


shufBut.onToggle(Boolean onOff){
	if(onOff) updateInfo("Shuffle: On");
	else updateInfo("Shuffle: Off");
}

repBut.onToggle(Boolean onOff){
	if(repBut.getCurCfgVal()==0) updateInfo("Repeat: Off");
	else if(repBut.getCurCfgVal()==1) updateInfo("Repeat: Playlist");
	else updateInfo("Repeat: Track");
}

muteBut.onToggle(Boolean onoff){
	//Cancel next volume info tip
	cancelNext=true;
	if(muteBut.getCurCfgVal()==0){
		updateInfo("Mute: Off");
	}
	else{
		updateInfo("Mute: On");
	}
}

updateInfo(String showThis){
	cancelStuff();

	//do stuff
	info_news.setText(showThis);
	showNews(true);
}

cancelStuff(){
	//cancel stuff
	fade.stop();
	goBack.stop();
	info_songticker.cancelTarget();
	info_news.cancelTarget();
}

showNews(boolean show){
	if(show){
		if(isShort){
			goBack.setDelay(200);
			isShort=false;
		}
		else{
			goBack.setDelay(1000);
		}
		
		if(!busyWithSeek) goBack.start();
		
		info_songticker.setAlpha(0);
		info_songticker.hide();
		info_news.setAlpha(255);
		info_news.show();
	}
	else{
		info_news.show();
		info_news.setTargetA(0);
		info_news.setTargetSpeed(fadeTime);
		info_news.gotoTarget();
		fade.start();
	}
}

fade.onTimer(){ //fade to songticker
	info_songticker.show();
	info_songticker.setAlpha(0);
	info_songticker.setTargetA(255);
	info_songticker.setTargetSpeed(fadeTime);
	info_songticker.gotoTarget();
	info_news.hide();
	fade.stop();
}

System.onVolumeChanged(int newvol){
	if(cancelNext){
		cancelNext=false;
	}
	else updateInfo(xlat_vol.getText() + " "+integerToString(newvol/255*100)+"%");
}

goBack.onTimer(){
	showNews(false);
	goBack.stop();
}
/*
sl_volume.onSetPosition(int newpos){
	updateInfo(xlat_vol.getText() + " "+integerToString(newpos/255*100)+"%");
}*/
sl_seeker.onSetPosition(int newpos){
	updateInfo("Seek: " + integerToTime(newpos/255*getPlayItemLength()) + "/" + integerToTime(getPlayItemLength()) +" ("+ integerToString(newpos/255*100)+"%)");
}

prev.onLeftClick(){
	isShort=true;
	updateInfo("Previous Track");
}
next.onLeftClick(){
	isShort=true;
	updateInfo("Next Track");
}

System.onStop(){
	updateInfo("Playback Stopped");
}
System.onPlay(){
	updateInfo("Playing");
}
System.onPause(){
	updateInfo("Playback Paused");
}
System.onResume(){
	updateInfo("Resuming Playback");
}
open.onLeftClick(){
	updateInfo("Open file(s)");
}


/*
This will check to see if mouse is still down... aka user busy with seek. If this isnt done, it will start flashing
*/
sl_volume.onLeftButtonUp(int x, int y){
	busyWithSeek=false;
	int newpos = System.getVolume();
	updateInfo(xlat_vol.getText() + " "+integerToString(newpos/255*100)+"%");
}
sl_volume.onLeftButtonDown(int x, int y){
	busyWithSeek=true;
}

sl_seeker.onLeftButtonUp(int x, int y){
	busyWithSeek=false;
	isShort=true;
	int newpos = sl_seeker.getPosition();
	updateInfo("Seek: " + integerToTime(newpos/255*getPlayItemLength()) + "/" + integerToTime(getPlayItemLength()) +" ("+ integerToString(newpos/255*100)+"%)");
}
sl_seeker.onLeftButtonDown(int x, int y){
	busyWithSeek=true;
}