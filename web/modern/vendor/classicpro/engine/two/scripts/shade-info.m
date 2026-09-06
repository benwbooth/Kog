#include <lib/std.mi>
#include <lib/pldir.mi>
#define fadeTime 0.3

Function updateInfo(String showThis);
Function showNews(boolean show);
Function cancelStuff();
//Function updateTickerScrolling();

Global Container mainContainer;
Global Layout mainLayout;

Global Group mainGroup, g_texttime;
Global Text info_news;
Global GuiObject info_songticker;
Global Timer fade, goBack;
Global Slider sl_volume, sl_seeker;
Global Button prev, next, open;
Global Boolean isShort, busyWithSeek, cancelNext;
Global Togglebutton shufBut, repBut, muteBut;
Global String def_Layout, def_Volume, def_Seeker, def_Prev, def_Next, def_Open, def_Shuf, def_Rep, def_Mute;
Global Text t_trackTime, t_totalTime, t_timeEvent;

System.onScriptLoaded(){
	mainGroup = getScriptGroup();
	info_songticker = mainGroup.getObject("shade.st.title");
	info_news = mainGroup.getObject("shade.st.news");
	g_texttime = mainGroup.getObject("shade.text.time");
	t_trackTime = g_texttime.getObject("shade.text.time.tracktime");
	t_totalTime = g_texttime.getObject("shade.text.time.totaltime");
	t_timeEvent = g_texttime.getObject("shade.text.time.trackevent");

	mainContainer = getContainer("main");
	mainLayout = mainContainer.getLayout("shade");
	



	fade = new Timer;
	fade.setDelay(300);
	goBack = new Timer;
	busyWithSeek=false;
	cancelNext=false;
	//updateTickerScrolling();
}

System.onscriptunloading(){
	delete fade;
	delete goBack;
}

mainLayout.onSetVisible(boolean onOff){
	if(onOff && prev==NULL){
		sl_volume = mainLayout.findObject("two.shade.volslider");
		sl_seeker = mainLayout.findObject("shade.seeker.slider.1");
		prev = mainLayout.findObject("shade.prev");
		next = mainLayout.findObject("shade.next");
		open = mainLayout.findObject("shade.open");
		shufBut = mainLayout.findObject("shade.shuf");
		repBut = mainLayout.findObject("shade.rep");
		muteBut = mainLayout.findObject("shade.mute");
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
		info_news.setTargetW(stringToInteger(info_news.getXmlParam("w")));
		info_news.setTargetSpeed(fadeTime);
		info_news.gotoTarget();
		fade.start();
	}
}

fade.onTimer(){ //fade to songticker
	info_songticker.show();
	info_songticker.setAlpha(0);
	info_songticker.setTargetA(255);
	info_songticker.setTargetW(stringToInteger(info_songticker.getXmlParam("w")));
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


/* Changing TickerScrolling via Config Attrib

ScrollingAttribute.onDataChanged(){
	updateTickerScrolling();
}

updateTickerScrolling(){
	if (info_songticker == NULL) return;
	
	if (songticker_scrolling_disabled_attrib.getData() == "1") info_songticker.setXMLParam("ticker", "off");
	if (songticker_style_modern_attrib.getData() == "1") info_songticker.setXMLParam("ticker", "bounce");
	if (songticker_style_old_attrib.getData() == "1") info_songticker.setXMLParam("ticker", "scroll");
}
 */
 
mainGroup.onResize(int x, int y, int w, int h){
	t_timeEvent.onTextChanged(t_timeEvent.getText());
}

t_timeEvent.onTextChanged(String newtxt){
	//debugstring(newtxt, 9);

	if(System.getPlayItemLength()<0){
		/*
		t_trackTime.hide();
		t_totalTime.setText(PlEdit.getLength(PlEdit.getCurrentIndex()));
		*/
		
		//t_totalTime.setText(System.integerToTime(System.getPlayItemLength()));
		
		//System.getPlayItemString()
		
		//PlEdit.getLength (PlEdit.getCurrentIndex());

		t_trackTime.setText("-");
		t_totalTime.setText("/ "+PlEdit.getLength(PlEdit.getCurrentIndex()));

	}
	else{
		t_trackTime.show();
		t_trackTime.setText(newtxt);
		t_totalTime.setText("/ "+System.integerToTime(System.getPlayItemLength()));
	}

	t_totalTime.setXmlParam("x", integerToString(t_trackTime.getTextWidth()-4+21));
	t_totalTime.setXmlParam("w", integerToString(t_totalTime.getTextWidth()));
	t_trackTime.setXmlParam("w", integerToString(t_trackTime.getTextWidth()));
	
	g_texttime.setXmlParam("x", integerToString(-(t_trackTime.getWidth()*t_trackTime.isVisible()+t_totalTime.getWidth()-4+21+stringToInteger(t_trackTime.getXmlParam("display")))));
	g_texttime.setXmlParam("w", integerToString(t_trackTime.getWidth()*t_trackTime.isVisible()+t_totalTime.getWidth()-5+21));
	
	info_songticker.setXmlParam("w", integerToString(-g_texttime.getWidth()-2-stringToInteger(t_trackTime.getXmlParam("display"))));
	info_news.setXmlParam("w", info_songticker.getXmlParam("w"));
	//setClipboardText(integerToString(t_nameTop.getLeft()));
	//t_totalTime.setText(System.integerToTime(System.getPlayItemLength()));
}
