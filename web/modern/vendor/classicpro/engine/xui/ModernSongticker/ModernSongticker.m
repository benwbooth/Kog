#include <lib/std.mi>
#include ../../scripts/attribs/init_songticker.m
#define fadeTime 0.3

Function updateInfo(String showThis);
Function showNews(boolean show);
Function cancelStuff();
Function updateTickerScrolling();

Global Container mainContainer;
Global Layout mainLayout;

Global Group mainGroup;
Global Text info_news;
Global GuiObject info_songticker;
Global Timer fade, goBack;
Global Slider sl_volume, sl_seeker;
Global Button prev, next, open;
Global Boolean isShort, busyWithSeek, cancelNext;
Global Togglebutton shufBut, repBut, muteBut;
Global String def_Layout, def_Volume, def_Seeker, def_Prev, def_Next, def_Open, def_Shuf, def_Rep, def_Mute;

System.onScriptLoaded(){
	mainGroup = getScriptGroup();
	info_songticker = mainGroup.findObject("m.st.ticker");
	info_news = mainGroup.findObject("m.st.news");

	initAttribs_Songticker();

	fade = new Timer;
	fade.setDelay(300);
	goBack = new Timer;
	busyWithSeek=false;
	cancelNext=false;
	updateTickerScrolling();
}

System.onShowLayout(Layout _layout){
	if(getParam()!=""){
		mainContainer = getContainer(getToken(getParam(), ";", 0));
		mainLayout = mainContainer.getLayout(getToken(getParam(), ";", 1));
	}
	else{
		mainContainer = getContainer(getToken(def_Layout, ";", 0));
		mainLayout = mainContainer.getLayout(getToken(def_Layout, ";", 1));
	}
	if(mainLayout==_layout){
		if(getParam()!=""){
			def_Volume = getToken(getParam(), ";", 2);
			def_Seeker = getToken(getParam(), ";", 3);
			def_Prev = getToken(getParam(), ";", 4);
			def_Next = getToken(getParam(), ";", 5);
			def_Open = getToken(getParam(), ";", 6);
			def_Shuf = getToken(getParam(), ";", 7);
			def_Rep = getToken(getParam(), ";", 8);
			def_Mute = getToken(getParam(), ";", 9);
		}
		sl_volume = mainLayout.findObject(def_Volume);
		sl_seeker = mainLayout.findObject(def_Seeker);
		prev = mainLayout.findObject(def_Prev);
		next = mainLayout.findObject(def_Next);
		open = mainLayout.findObject(def_Open);
		shufBut = mainLayout.findObject(def_Shuf);
		repBut = mainLayout.findObject(def_Rep);
		muteBut = mainLayout.findObject(def_Mute);
	}
}

System.onscriptunloading(){
	delete fade;
	delete goBack;
}

System.onSetXuiParam(String param, String value) {
	if(getParam()!="") return;
	if(strlower(param) == "id_layout"){
		def_Layout = value;
	}
	else if(strlower(param) == "id_volume"){
		def_Volume = value;
	}
	else if(strlower(param) == "id_seeker"){
		def_Seeker = value;
	}
	else if(strlower(param) == "id_prev"){
		def_Prev = value;
	}
	else if(strlower(param) == "id_next"){
		def_Next = value;
	}
	else if(strlower(param) == "id_open"){
		def_Open = value;
	}
	else if(strlower(param) == "id_shuf"){
		def_Shuf = value;
	}
	else if(strlower(param) == "id_rep"){
		def_Rep = value;
	}
	else if(strlower(param) == "id_mute"){
		def_Mute = value;
	}
	else if(strlower(param) == "ghost_all"){
		info_songticker.setXmlParam("ghost", value);
		info_news.setXmlParam("ghost", value);
	}
	else if(strlower(param) == "showlen_all"){
		info_songticker.setXmlParam("showlen", value);
	}
	else{
		info_songticker.setXmlParam(param, value);
		info_news.setXmlParam(param, value);
	}
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
	if(mainLayout!=mainContainer.getCurLayout()) return;
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
	else updateInfo(System.translate("Volume") +": "+integerToString(newvol/255*100)+"%");
}

goBack.onTimer(){
	showNews(false);
	goBack.stop();
}

sl_seeker.onSetPosition(int newpos){
	updateInfo(System.translate("Seek") +": "+integerToTime(newpos/255*getPlayItemLength()) + "/" + integerToTime(getPlayItemLength()) +" ("+ integerToString(newpos/255*100)+"%)");
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
	updateInfo(System.translate("Volume") +": "+integerToString(newpos/255*100)+"%");
}
sl_volume.onLeftButtonDown(int x, int y){
	busyWithSeek=true;
}

sl_seeker.onLeftButtonUp(int x, int y){
	busyWithSeek=false;
	isShort=true;
	int newpos = sl_seeker.getPosition();
	updateInfo(System.translate("Seek") +": "+integerToTime(newpos/255*getPlayItemLength()) + "/" + integerToTime(getPlayItemLength()) +" ("+ integerToString(newpos/255*100)+"%)");
}
sl_seeker.onLeftButtonDown(int x, int y){
	busyWithSeek=true;
}


/* Changing TickerScrolling via Config Attrib */

ScrollingAttribute.onDataChanged(){
	updateTickerScrolling();
}

updateTickerScrolling(){
	if (info_songticker == NULL) return;
	
	if (songticker_scrolling_disabled_attrib.getData() == "1") info_songticker.setXMLParam("ticker", "off");
	if (songticker_style_modern_attrib.getData() == "1") info_songticker.setXMLParam("ticker", "bounce");
	if (songticker_style_old_attrib.getData() == "1") info_songticker.setXMLParam("ticker", "scroll");
}
